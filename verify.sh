#!/usr/bin/env bash
set -eo pipefail

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo "🔍 Starting local pre-deployment validation..."

# 1. Check Cargo
echo -e "\n1. Checking Cargo/Rust syntax..."
if ! command -v cargo &> /dev/null; then
    echo -e "${YELLOW}⚠️  cargo command not found. Skipping Rust compilation check.${NC}"
else
    if cargo check; then
        echo -e "${GREEN}✓ Rust compilation check passed!${NC}"
    else
        echo -e "${RED}✗ Rust compilation check failed!${NC}"
        exit 1
    fi
fi

# 2. Check Docker Compose
echo -e "\n2. Checking Docker Compose configuration..."
if ! command -v docker &> /dev/null; then
    echo -e "${YELLOW}⚠️  docker command not found. Skipping Docker Compose check.${NC}"
else
    if docker compose config > /dev/null; then
        echo -e "${GREEN}✓ Docker Compose config validation passed!${NC}"
    else
        echo -e "${RED}✗ Docker Compose config validation failed!${NC}"
        exit 1
    fi
fi

# 3. Check Dockerfile
echo -e "\n3. Checking Dockerfile syntax..."
if ! command -v docker &> /dev/null; then
    echo -e "${YELLOW}⚠️  docker command not found. Skipping Dockerfile check.${NC}"
else
    if docker build --check . > /dev/null 2>&1; then
        echo -e "${GREEN}✓ Dockerfile syntax check passed!${NC}"
    else
        # Fallback if --check is not supported on older docker versions
        echo -e "${YELLOW}⚠️  docker build --check not supported or failed. Doing quiet compose build check...${NC}"
        if docker compose build --quiet api; then
             echo -e "${GREEN}✓ Dockerfile build validation passed!${NC}"
        else
             echo -e "${RED}✗ Dockerfile validation failed!${NC}"
             exit 1
        fi
    fi
fi

echo -e "\n${GREEN}🚀 All local checks passed! You are safe to push to GitHub.${NC}"
