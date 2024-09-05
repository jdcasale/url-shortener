Persistence \[Draft\]
===========

Right now, the service that drops all of its state on the floor on restart -- this is obviously not desirable, as users
will expect some sort of formal contract for the lifetimes of their shortened links, and "idk my bff jill" is not an
acceptable contract.

We could offload this to a cloud service which is
responsible for fault tolerance, redundancy, etc, but this kind of side-steps the core engineering exercise here.
(this whole thing could be done in a few lines of lambda + kvs, lame, plus cloud services are expensive)

So we need a mechanism for persisting link state beyond restart. This can be broken down into several problems:

### Consistency Model
Every time we create a link, we want not only to store a persistent record, but we want the system as a whole to recognize
this write. Once I write a url, we should be able to guarantee that readers can see it. We could make some sacrifices here 
and allow eventual consistency on the order of seconds, but honestly it doesn't make things much easier or cheaper, and 
greatly complicates the forthcoming database design.



  

Solution? Roll our own database *gasp*.
Ok so it's not quite as bad as it seems. We'll rely on RocksDB for the primitive KV store, RAFT for synchronization,
and we'll cheat just a little bit by using a blob store.

### Requirements
OK, so ~~here's the earth, _chillin_~~ the basic goal is to provide a key-value store that meets the following requirements:
- Writes are fast (thousands of writes/sec, sub-second latency)
- Reads are fast(er) (tens of ms latency, overall volume will be much higher than writes, but is extremely cacheable)
- Storage size is (kind of) unbounded
  - We may not ever clean up old links (maybe we do garbage collection for a free tier, but paid users will want perma-links etc)
  - If we were TinyURL, we'd end up with TB of storage after a few years
  - Cold storage is kind of ok, but cold here still means subsecond latency, users do not want to wait seconds for a url expansion
  - 

### ~~How do I shot web?~~ Design
#### Writes
We shard writes to one of N shards. Initially N=1. Each shard is owned by one leader, which uses RAFT to synchronize writes
to all of the other nodes. The reason we shard is so that we can allow multiple writers, rather than relying on the write
throughput of a single node for the whole cluster. This means that each node will be running N RAFT state machines, but
this is really just an implementation detail, and for what it's worth, in early benchmarking, we can achieve 8k writes/sec
on my laptop with one single node -- this would correspond to ~100x Tinyurls' monthly web traffic, and their traffic is almost
certainly read-heavy.

The raft long is persisted on each node via Rocksdb. This would become a major problem over time, as the performance of
a rocks table can slow down over time. HOWEVER, we do not keep all of the data in a single rocksdb table, nor do we want to.

Once the rocks table hits a certain size, we send an UPLOAD message to the cluster. This tells the cluster to take all
of the data currently in the rocks instance and export it to a blob store. We scribble down a note in the state machine's
metadata table to note this.

While this is happening, we continue to accept writes, but once it's done, we sent through a REDISTRIBUTE message with
the chunk id. Chunk ids are distributed between nodes via a consistent hashing scheme, and each node has a thread that
keeps an eye on the node's local chunk store and the metadata table, and downloads chunks when necessary. It then
updates the state machine to notify that the chunk has been downloaded. Each chunk is assigned to at least 2 nodes for
redundancy, and if a node fails to heartbeat to the raft store, we note this so that the remaining nodes can re-download
the applicable chunks so that we never end up with missing data
^^ REDO THIS EXPLANATION IT'S TERRIBLE AND I'M TIRED

Once the chunk has been downloaded to it's required redundancy level, we nuke the associated IDs from the state machine.

This process gives us a couple of benefits:
- It imposes a ceiling on the size of the state machine.
  - Otherwise:
    - snapshots would eventually begin to fail
    - startups will take longer and longer over time
    - reads/writes would get slower and slower over time
- It enables us to persist all of the data to a blob store, which makes disaster recovery possible
- It enables us to compact chunks in order to redistribute keys and ameliorate issues with hot-spotting 



#### Reads
Each node serves read traffic, and has an in-memory cache. My laptop serves ~90k cached reads/sec -- if we can get
even a 20th of this we're easily in the clear.

The complexity comes in two places:
- Consistent hashing
- The chunk store

#### Consistent Hashing
Reads get routed to the appropriate node via a hashing scheme based on the chunk metadata in the state machine.

#### Chunk store
The chunk store downloads chunks from the blob store and then serves the values in these chunks.

