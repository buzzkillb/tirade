-- SQL queries to verify USDC balance change tracking in the database

-- 1. Check if positions table has USDC columns
PRAGMA table_info(positions);

-- 2. Check all positions with USDC data
SELECT 
    id,
    pair,
    position_type,
    entry_price,
    exit_price,
    quantity,
    usdc_spent,
    usdc_received,
    pnl,
    (CASE 
        WHEN usdc_received IS NOT NULL AND usdc_spent IS NOT NULL 
        THEN usdc_received - ABS(usdc_spent)
        ELSE pnl 
    END) as calculated_usdc_pnl,
    status,
    created_at
FROM positions 
ORDER BY created_at DESC 
LIMIT 10;

-- 3. Check USDC-based vs price-based PnL calculation
SELECT 
    'USDC-based trades' as type,
    COUNT(*) as count,
    SUM(usdc_received - ABS(usdc_spent)) as total_pnl
FROM positions 
WHERE status = 'closed' 
  AND usdc_received IS NOT NULL 
  AND usdc_spent IS NOT NULL

UNION ALL

SELECT 
    'Price-based trades' as type,
    COUNT(*) as count,
    SUM(pnl) as total_pnl
FROM positions 
WHERE status = 'closed' 
  AND (usdc_received IS NULL OR usdc_spent IS NULL);

-- 4. Performance metrics calculation (same as dashboard)
SELECT 
    COALESCE(SUM(CASE 
        WHEN usdc_received IS NOT NULL AND usdc_spent IS NOT NULL 
        THEN usdc_received - ABS(usdc_spent)
        ELSE pnl 
    END), 0.0) as total_pnl,
    COUNT(CASE WHEN usdc_received IS NOT NULL AND usdc_spent IS NOT NULL THEN 1 END) as usdc_based_trades,
    COUNT(*) as total_trades
FROM positions 
WHERE status = 'closed';