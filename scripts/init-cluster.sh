#!/bin/sh
set -ex

# Wait for the first node to be ready
sleep 2

# Add learners one at a time
curl -X POST http://url-shortener-1:8080/cluster/add-learner \
  -H "Content-Type: application/json" \
  -d '[2, "url-shortener-2:8080"]'

sleep 1

curl -X POST http://url-shortener-1:8080/cluster/add-learner \
  -H "Content-Type: application/json" \
  -d '[3, "url-shortener-3:8080"]'

sleep 1

# Change membership to include all nodes
curl -X POST http://url-shortener-1:8080/cluster/change-membership \
  -H "Content-Type: application/json" \
  -d '[1, 2, 3]'

# Wait for membership change to propagate
sleep 2

# Test the cluster with a write operation
curl -X POST http://url-shortener-1:8080/submit \
  -H "Content-Type: application/json" \
  -d '{"long_url": "https://www.google.com/search/abcdefg"}'

echo "Cluster initialized."
