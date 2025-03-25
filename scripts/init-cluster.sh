#!/bin/sh
set -ex

curl -X POST http://url-shortener-1:8080/cluster/add-learner \
  -H "Content-Type: application/json" \
  -d '[2, "url-shortener-2:8080"]'

curl -X POST http://url-shortener-1:8080/cluster/add-learner \
  -H "Content-Type: application/json" \
  -d '[3, "url-shortener-3:8080"]'

curl -X POST http://url-shortener-2:8080/cluster/add-learner \
  -H "Content-Type: application/json" \
  -d '[3, "url-shortener-3:8080"]'

curl -X POST http://url-shortener-1:8080/cluster/change-membership \
  -H "Content-Type: application/json" \
  -d '[1, 2, 3]'
curl -X POST http://url-shortener-2:8080/cluster/change-membership \
  -H "Content-Type: application/json" \
  -d '[1, 2, 3]'
curl -X POST http://url-shortener-3:8080/cluster/change-membership \
  -H "Content-Type: application/json" \
  -d '[1, 2, 3]'

curl -X POST  http://url-shortener-1:8080/submit \
  -H "Content-Type: application/json" \
  -d '{"long_url": "https://www.google.com/search/abcdefg"}'
curl -X POST  http://url-shortener-2:8080/submit \
  -H "Content-Type: application/json" \
  -d '{"long_url": "https://www.google.com/search/abcdefg"}'
curl -X POST  http://url-shortener-3:8080/submit \
  -H "Content-Type: application/json" \
  -d '{"long_url": "https://www.google.com/search/abcdefg"}'

curl -X GET  http://url-shortener-1:8080/lookup/abcdefg

curl -X GET  http://url-shortener-2:8080/lookup/abcdefg

curl -X GET  http://url-shortener-3:8080/lookup/abcdefg

echo "Cluster initialized."
