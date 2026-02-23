#!/bin/bash
# GitHub Setup Script for acdc-botnet
# Run this script with your GitHub token to complete the setup

set -e

# Check for token
if [ -z "$GITHUB_TOKEN" ]; then
  echo "❌ Error: GITHUB_TOKEN environment variable not set"
  echo ""
  echo "To set your token:"
  echo "  export GITHUB_TOKEN='your_github_personal_access_token'"
  echo ""
  echo "To create a token:"
  echo "  1. Go to: https://github.com/settings/tokens"
  echo "  2. Click 'Generate new token (classic)'"
  echo "  3. Select scopes: repo, admin:org"
  echo "  4. Copy the token and export it"
  exit 1
fi

echo "🚀 Creating GitHub repository: alpha-delta-network/acdc-botnet"

# Create repository
RESPONSE=$(curl -s -X POST https://api.github.com/orgs/alpha-delta-network/repos \
  -H "Authorization: token ${GITHUB_TOKEN}" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "acdc-botnet",
    "description": "Distributed bot testing infrastructure for Alpha/Delta protocol. 31 scenarios, 99% coverage, formal correctness.",
    "private": false,
    "has_issues": true,
    "has_wiki": true,
    "has_projects": true,
    "homepage": "https://source.ac-dc.network/alpha-delta-network/acdc-botnet"
  }')

# Check if creation was successful
if echo "$RESPONSE" | grep -q '"full_name"'; then
  echo "✅ Repository created successfully!"
  echo ""
  
  # Add GitHub remote
  cd /home/devops/working-repos/acdc-botnet
  if ! git remote | grep -q "github"; then
    git remote add github https://github.com/alpha-delta-network/acdc-botnet.git
    echo "✅ GitHub remote added"
  fi
  
  # Push to GitHub
  echo "📤 Pushing to GitHub..."
  git push github master
  
  echo ""
  echo "✅ Setup complete!"
  echo "📍 GitHub: https://github.com/alpha-delta-network/acdc-botnet"
  echo "📍 Forgejo: https://source.ac-dc.network/alpha-delta-network/acdc-botnet"
  echo "📍 Radicle: rad:z2WYmpZk4rXZ3K3ToSF6ndfuRNNGa"
else
  echo "❌ Error creating repository:"
  echo "$RESPONSE" | jq '.'
  exit 1
fi
