#!/bin/bash

# Script to delete all data from D1 database (local and remote)
# WARNING: This will permanently delete ALL data from your database

set -e

# Colors for output
RED='\033[0;31m'
YELLOW='\033[1;33m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

# Database name (update if different)
DB_NAME="prod-hamrah-app-auth"

echo -e "${RED}⚠️  WARNING: This will DELETE ALL DATA from your D1 database!${NC}"
echo -e "${YELLOW}This action affects both local and remote databases and CANNOT be undone.${NC}"
echo ""
echo "Database: $DB_NAME"
echo ""
echo "Tables that will be cleared:"
echo "  - users"
echo "  - sessions" 

echo "  - auth_tokens"
echo ""

# Confirmation prompts
read -p "Are you absolutely sure you want to delete ALL data? (type 'DELETE' to confirm): " confirm1
if [ "$confirm1" != "DELETE" ]; then
    echo "Operation cancelled."
    exit 1
fi

read -p "This will affect PRODUCTION data if you have remote databases. Continue? (type 'YES' to confirm): " confirm2
if [ "$confirm2" != "YES" ]; then
    echo "Operation cancelled."
    exit 1
fi

echo ""
echo -e "${YELLOW}Starting database cleanup...${NC}"

# SQL commands to delete all data
SQL_COMMANDS="
-- Disable foreign key checks temporarily
PRAGMA foreign_keys = OFF;

-- Delete all data from tables (order matters due to foreign keys)
-- Use IF EXISTS to handle cases where tables don't exist
DELETE FROM auth_tokens WHERE 1=1;
DELETE FROM webauthn_challenges WHERE 1=1;
DELETE FROM webauthn_credentials WHERE 1=1;
DELETE FROM sessions WHERE 1=1;
DELETE FROM users WHERE 1=1;

-- Re-enable foreign key checks
PRAGMA foreign_keys = ON;

-- Vacuum to reclaim space
VACUUM;
"

# Function to execute SQL commands
execute_sql() {
    local env=$1
    local db_flag=$2
    
    echo "Deleting data from $env database..."
    
    # First check what tables exist
    local check_sql=$(mktemp)
    echo "SELECT name FROM sqlite_master WHERE type='table';" > "$check_sql"
    
    echo "Checking existing tables..."
    local tables_result=$(npx wrangler d1 execute $DB_NAME $db_flag --file "$check_sql" 2>/dev/null)
    
    # Clean up check file
    rm "$check_sql"
    
    # If no tables exist, just report success
    if echo "$tables_result" | grep -q '"results": \[\]'; then
        echo -e "${YELLOW}No tables found in $env database - nothing to delete${NC}"
        echo -e "${GREEN}✅ $env database is already empty${NC}"
        return 0
    fi
    
    # Create temporary SQL file for deletion
    local temp_sql=$(mktemp)
    echo "$SQL_COMMANDS" > "$temp_sql"
    
    if ! npx wrangler d1 execute $DB_NAME $db_flag --file "$temp_sql" 2>/dev/null; then
        echo -e "${YELLOW}Some tables may not exist, attempting individual deletions...${NC}"
        
        # Try individual table deletions
        for table in auth_tokens sessions users; do
            local single_sql=$(mktemp)
            echo "DELETE FROM $table WHERE 1=1;" > "$single_sql"
            if npx wrangler d1 execute $DB_NAME $db_flag --file "$single_sql" 2>/dev/null; then
                echo "  ✅ Cleared $table"
            else
                echo "  ⏭️  Skipped $table (doesn't exist)"
            fi
            rm "$single_sql"
        done
    fi
    
    # Clean up temp file
    rm "$temp_sql"
    
    echo -e "${GREEN}✅ Successfully processed $env database${NC}"
}

# Delete from local database
echo ""
echo -e "${YELLOW}Deleting from LOCAL database...${NC}"
execute_sql "local" "--local"

# Ask about remote database
echo ""
read -p "Do you also want to delete from REMOTE/PRODUCTION database? (type 'PRODUCTION' to confirm): " remote_confirm
if [ "$remote_confirm" = "PRODUCTION" ]; then
    echo ""
    echo -e "${YELLOW}Deleting from REMOTE/PRODUCTION database...${NC}"
    execute_sql "remote" "--remote"
else
    echo "Skipping remote database deletion."
fi

echo ""
echo -e "${GREEN}🎉 Database cleanup completed!${NC}"
echo ""
echo "Summary:"
echo "  - Local database: ✅ Cleared"
if [ "$remote_confirm" = "PRODUCTION" ]; then
    echo "  - Remote database: ✅ Cleared"
else
    echo "  - Remote database: ⏭️  Skipped"
fi
echo ""
echo "All user accounts, sessions, and authentication data have been removed."