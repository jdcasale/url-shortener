Url Shortener
=============
_**A very incomplete, unserious project. Have low expectations for both quality and features**_ 

A web service that implements URL shortening functionality akin to Bitly, Tinyurl etc. (Intentionally) Massively 
overengineered -- this whole thing could be done by stitching some lamdas to a cache + kv store, but I wanted to have fun.
It is certainly NOT the academically-correct solution to this problem -- I just wanted to fool around with raft and 
familiarize myself with rust.


Deployed via fly.io because navigating AWS deployment for a personal project is bleh.
Overall goal is to cheap -- more precisely, to be able to run Tinyurl's global traffic off of a single-digit number of fly machines.


TODO
----
- [x] Persistence -- ~right now, everything is in-memory, but we'll need to persist urls so that we don't wipe all created urls on restart~ Writes are strongly-consistent via RAFT and written to an on-disk RocksDB store. (done)
- [x] Distribution -- writes to one node should be reflected in reads to any node  (done)
- [ ] Sharding -- goes hand-in-hand with persistence. We don't want to have one giant, replicated database with a single keyspace for obvious reasons 
- [ ] Authentication -- there is no authentication, nor is there any user tracking. In order to implement billing, attribution, etc this is obviously necessary.
