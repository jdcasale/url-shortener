#!/bin/sh
set -ex

curl -X POST http://url-shortener-1:8080/cluster/add-learner \
  -H "Content-Type: application/json" \
  -d '[2, "url-shortener-2:21001"]'

curl -X POST http://url-shortener-1:8080/cluster/add-learner \
  -H "Content-Type: application/json" \
  -d '[3, "url-shortener-3:21001"]'

curl -X POST http://url-shortener-1:8080/cluster/change-membership \
  -H "Content-Type: application/json" \
  -d '[1, 2, 3]'

echo "Cluster initialized."
