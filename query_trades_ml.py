#!/usr/bin/env python3
"""
ML Trade Data Query Script

This script allows you to manually check and verify that machine learning trade data
is being stored correctly in the database. It provides various query options and
detailed analysis of the ML trade history.

Usage:
    python query_trades_ml.py --pair SOLUSDC --limit 20
    python query_trades_ml.py --pair SOLUSDC --stats
    python query_trades_ml.py --pair SOLUSDC --verify
    python query_trades_ml.py --pair SOLUSDC --export csv
"""

import argparse
import json
import sys
import requests
from datetime import datetime, timezone
from typing import Dict, List, Optional, Any
import csv
from pathlib import Path

# Configuration
DEFAULT_DATABASE_URL = "http://localhost:8080"
DEFAULT_PAIR = "SOLUSDC"

class MLTradeQuery:
    def __init__(self, database_url: str = DEFAULT_DATABASE_URL):
        self.database_url = database_url.rstrip('/')
        self.session = requests.Session()
        self.session.timeout = 10

    def get_ml_trades(self, pair: str, limit: int = 50) -> List[Dict[str, Any]]:
        """Fetch ML trade history for a specific pair."""
        url = f"{self.database_url}/ml/trades/{pair}"
        params = {"limit": limit}
        
        try:
            response = self.session.get(url, params=params)
            response.raise_for_status()
            
            data = response.json()
            if data.get("success") and "data" in data:
                return data["data"]
            else:
                print(f"❌ Error: {data.get('message', 'Unknown error')}")
                return []
                
        except requests.exceptions.RequestException as e:
            print(f"❌ Network error: {e}")
            return []
        except json.JSONDecodeError as e:
            print(f"❌ JSON decode error: {e}")
            return []

    def get_ml_stats(self, pair: str) -> Optional[Dict[str, Any]]:
        """Fetch ML trade statistics for a specific pair."""
        url = f"{self.database_url}/ml/stats/{pair}"
        
        try:
            response = self.session.get(url)
            response.raise_for_status()
            
            data = response.json()
            if data.get("success") and "data" in data:
                return data["data"]
            else:
                print(f"❌ Error: {data.get('message', 'Unknown error')}")
                return None
                
        except requests.exceptions.RequestException as e:
            print(f"❌ Network error: {e}")
            return None
        except json.JSONDecodeError as e:
            print(f"❌ JSON decode error: {e}")
            return None

    def get_ml_status(self) -> Optional[Dict[str, Any]]:
        """Fetch overall ML status."""
        url = f"{self.database_url}/ml/status"
        
        try:
            response = self.session.get(url)
            response.raise_for_status()
            
            data = response.json()
            if data.get("success") and "data" in data:
                return data["data"]
            else:
                print(f"❌ Error: {data.get('message', 'Unknown error')}")
                return None
                
        except requests.exceptions.RequestException as e:
            print(f"❌ Network error: {e}")
            return None
        except json.JSONDecodeError as e:
            print(f"❌ JSON decode error: {e}")
            return None

    def format_trade(self, trade: Dict[str, Any]) -> str:
        """Format a single trade for display."""
        entry_time = datetime.fromisoformat(trade["entry_time"].replace('Z', '+00:00'))
        exit_time = datetime.fromisoformat(trade["exit_time"].replace('Z', '+00:00'))
        created_at = datetime.fromisoformat(trade["created_at"].replace('Z', '+00:00'))
        
        pnl_percent = trade["pnl"] * 100
        duration_minutes = trade["duration_seconds"] / 60
        
        status_emoji = "✅" if trade["success"] else "❌"
        pnl_emoji = "💰" if trade["pnl"] > 0 else "💸" if trade["pnl"] < 0 else "➡️"
        
        return (
            f"{status_emoji} {pnl_emoji} {trade['pair']} | "
            f"Entry: ${trade['entry_price']:.4f} | "
            f"Exit: ${trade['exit_price']:.4f} | "
            f"PnL: {pnl_percent:+.2f}% | "
            f"Duration: {duration_minutes:.1f}m | "
            f"Regime: {trade['market_regime']} | "
            f"Trend: {trade['trend_strength']:.3f} | "
            f"Vol: {trade['volatility']:.3f} | "
            f"Time: {entry_time.strftime('%Y-%m-%d %H:%M:%S')}"
        )

    def display_trades(self, trades: List[Dict[str, Any]], show_details: bool = False):
        """Display trades in a formatted way."""
        if not trades:
            print("📭 No ML trades found for this pair.")
            return

        print(f"\n🤖 ML Trade History ({len(trades)} trades):")
        print("=" * 120)
        
        for i, trade in enumerate(trades, 1):
            print(f"{i:2d}. {self.format_trade(trade)}")
            
            if show_details:
                print(f"    ID: {trade['id']}")
                print(f"    Created: {datetime.fromisoformat(trade['created_at'].replace('Z', '+00:00')).strftime('%Y-%m-%d %H:%M:%S')}")
                print()

    def display_stats(self, stats: Dict[str, Any]):
        """Display ML trade statistics."""
        print(f"\n📊 ML Trade Statistics:")
        print("=" * 50)
        print(f"Total Trades: {stats['total_trades']}")
        print(f"Win Rate: {stats['win_rate'] * 100:.1f}%")
        print(f"Average PnL: {stats['avg_pnl'] * 100:+.2f}%")
        print(f"Average Win: {stats['avg_win'] * 100:+.2f}%")
        print(f"Average Loss: {stats['avg_loss'] * 100:+.2f}%")

    def display_ml_status(self, status: Dict[str, Any]):
        """Display overall ML status."""
        print(f"\n🤖 ML System Status:")
        print("=" * 50)
        print(f"Enabled: {'✅' if status['enabled'] else '❌'}")
        print(f"Min Confidence: {status['min_confidence'] * 100:.1f}%")
        print(f"Max Position Size: {status['max_position_size'] * 100:.1f}%")
        print(f"Total Trades: {status['total_trades']}")
        print(f"Win Rate: {status['win_rate'] * 100:.1f}%")
        print(f"Average PnL: {status['avg_pnl'] * 100:+.2f}%")

    def verify_trade_data(self, trades: List[Dict[str, Any]]) -> Dict[str, Any]:
        """Verify the integrity of trade data."""
        if not trades:
            return {"valid": False, "errors": ["No trades found"]}

        errors = []
        warnings = []
        
        for i, trade in enumerate(trades):
            # Check required fields
            required_fields = ["id", "pair", "entry_price", "exit_price", "pnl", 
                             "duration_seconds", "entry_time", "exit_time", "success",
                             "market_regime", "trend_strength", "volatility", "created_at"]
            
            for field in required_fields:
                if field not in trade:
                    errors.append(f"Trade {i+1}: Missing required field '{field}'")
            
            # Check data types and ranges
            if "entry_price" in trade and trade["entry_price"] <= 0:
                errors.append(f"Trade {i+1}: Invalid entry_price ({trade['entry_price']})")
            
            if "exit_price" in trade and trade["exit_price"] <= 0:
                errors.append(f"Trade {i+1}: Invalid exit_price ({trade['exit_price']})")
            
            if "pnl" in trade and not isinstance(trade["pnl"], (int, float)):
                errors.append(f"Trade {i+1}: Invalid pnl type ({type(trade['pnl'])})")
            
            if "duration_seconds" in trade and trade["duration_seconds"] < 0:
                warnings.append(f"Trade {i+1}: Negative duration ({trade['duration_seconds']}s)")
            
            if "trend_strength" in trade and not (0 <= trade["trend_strength"] <= 1):
                warnings.append(f"Trade {i+1}: Trend strength out of range ({trade['trend_strength']})")
            
            if "volatility" in trade and trade["volatility"] < 0:
                warnings.append(f"Trade {i+1}: Negative volatility ({trade['volatility']})")
            
            # Check market regime values
            valid_regimes = ["Consolidating", "Trending", "Volatile"]
            if "market_regime" in trade and trade["market_regime"] not in valid_regimes:
                warnings.append(f"Trade {i+1}: Unknown market regime ({trade['market_regime']})")
            
            # Check time consistency
            if "entry_time" in trade and "exit_time" in trade:
                try:
                    entry_time = datetime.fromisoformat(trade["entry_time"].replace('Z', '+00:00'))
                    exit_time = datetime.fromisoformat(trade["exit_time"].replace('Z', '+00:00'))
                    
                    if exit_time <= entry_time:
                        errors.append(f"Trade {i+1}: Exit time before or equal to entry time")
                    
                    if "duration_seconds" in trade:
                        calculated_duration = (exit_time - entry_time).total_seconds()
                        if abs(calculated_duration - trade["duration_seconds"]) > 1:
                            warnings.append(f"Trade {i+1}: Duration mismatch (calculated: {calculated_duration:.0f}s, stored: {trade['duration_seconds']}s)")
                            
                except ValueError as e:
                    errors.append(f"Trade {i+1}: Invalid datetime format: {e}")

        return {
            "valid": len(errors) == 0,
            "errors": errors,
            "warnings": warnings,
            "total_trades": len(trades)
        }

    def export_trades(self, trades: List[Dict[str, Any]], format_type: str, filename: Optional[str] = None):
        """Export trades to CSV or JSON format."""
        if not trades:
            print("📭 No trades to export.")
            return

        if filename is None:
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
            filename = f"ml_trades_{trades[0]['pair']}_{timestamp}"

        if format_type.lower() == "csv":
            filename += ".csv"
            with open(filename, 'w', newline='', encoding='utf-8') as csvfile:
                fieldnames = ["id", "pair", "entry_price", "exit_price", "pnl", "duration_seconds",
                             "entry_time", "exit_time", "success", "market_regime", "trend_strength",
                             "volatility", "created_at"]
                writer = csv.DictWriter(csvfile, fieldnames=fieldnames)
                writer.writeheader()
                for trade in trades:
                    writer.writerow(trade)
            print(f"📁 Exported {len(trades)} trades to {filename}")
            
        elif format_type.lower() == "json":
            filename += ".json"
            with open(filename, 'w', encoding='utf-8') as jsonfile:
                json.dump(trades, jsonfile, indent=2, default=str)
            print(f"📁 Exported {len(trades)} trades to {filename}")
        
        else:
            print(f"❌ Unsupported export format: {format_type}")

def main():
    parser = argparse.ArgumentParser(description="Query ML trade data from the database")
    parser.add_argument("--pair", default=DEFAULT_PAIR, help="Trading pair (default: SOLUSDC)")
    parser.add_argument("--limit", type=int, default=20, help="Number of trades to fetch (default: 20)")
    parser.add_argument("--database-url", default=DEFAULT_DATABASE_URL, help="Database service URL")
    parser.add_argument("--stats", action="store_true", help="Show ML trade statistics")
    parser.add_argument("--status", action="store_true", help="Show ML system status")
    parser.add_argument("--verify", action="store_true", help="Verify trade data integrity")
    parser.add_argument("--details", action="store_true", help="Show detailed trade information")
    parser.add_argument("--export", choices=["csv", "json"], help="Export trades to file")
    parser.add_argument("--output", help="Output filename for export")
    
    args = parser.parse_args()
    
    # Initialize query client
    client = MLTradeQuery(args.database_url)
    
    print(f"🔍 Querying ML trades for {args.pair}...")
    print(f"🌐 Database URL: {args.database_url}")
    
    # Get ML trades
    trades = client.get_ml_trades(args.pair, args.limit)
    
    if not trades:
        print(f"❌ No ML trades found for {args.pair}")
        sys.exit(1)
    
    # Display trades
    client.display_trades(trades, args.details)
    
    # Show statistics if requested
    if args.stats:
        stats = client.get_ml_stats(args.pair)
        if stats:
            client.display_stats(stats)
    
    # Show ML status if requested
    if args.status:
        status = client.get_ml_status()
        if status:
            client.display_ml_status(status)
    
    # Verify data integrity if requested
    if args.verify:
        print(f"\n🔍 Verifying trade data integrity...")
        verification = client.verify_trade_data(trades)
        
        if verification["valid"]:
            print("✅ All trade data is valid!")
        else:
            print("❌ Data validation errors found:")
            for error in verification["errors"]:
                print(f"   • {error}")
        
        if verification["warnings"]:
            print("\n⚠️  Warnings:")
            for warning in verification["warnings"]:
                print(f"   • {warning}")
        
        print(f"\n📊 Verification Summary:")
        print(f"   Total trades checked: {verification['total_trades']}")
        print(f"   Errors: {len(verification['errors'])}")
        print(f"   Warnings: {len(verification['warnings'])}")
    
    # Export if requested
    if args.export:
        client.export_trades(trades, args.export, args.output)
    
    print(f"\n✅ Query completed successfully!")

if __name__ == "__main__":
    main() 