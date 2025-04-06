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
and we'll cheat just a little bit by using a blob store for backups and shard distribution.

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

##### v0
There's a lot of complexity baked into the eventual design. We'll get there incrementally. The first iteration uses a single
RAFT store (and thus a single writer). There is no sharding or partitioning. We just write to a single,
strongly-consistent store. The raft log is durable because of the RocksDB table that backs the raft implementation,
but the k-v store itself s in memory. There are no backups, no sharding, and as the table grows large, 
memory footprint grows and snapshots become intractible because they involve shipping the entire database.

##### v1
We introduce the notion of chunks. We separate the db into a history key-value store and a writes key-value store.
Occasionally*, we take everything in the 'writes' store, freeze it, give it a 'chunk id', store it in an 
external (s3) blob store. We then move those values from 'writes' to 'history', clear out 'writes', and scribble down
the chunk id in our state machine. We no longer store historical data in the raft snapshots/log because this can be
reconstructed deterministically from the chunk ids in the metadata. This way, we bound the growth of our raft log and
snapshots 

WHen a node goes to restore from a snapshot, it downloads the relevant blob and merges that into its history store.

##### v2
Migrate the in-memory historical store to rocks. We are then no longer limited to the size of a kvs that we can fit in 
memory.

##### v4
Instead of all recent writes going into the same chunk and having a monolithic rocks lookup, we bucket the writes into
some large number of bucket keys and give each of those a chunk-id salted with its bucket-key. We can then keep track
of a mapping between nodes and assigned chunk keys (consistent hashing) and distribute chunks accordingly. When a new
node joins the ring, we can reassign and redistribute chunks. Reads can be forwarded to the appropriate node based on
this state.

This way, we don't need to keep a full copy of the data on each node. 


##### v99 (maybe)
We introduce multiple writers. Currently, any write that is directed to a node that is not the leader must be forwarded
to the leader, who then needs to chatter back to the rest of the cluster via RAFT. This extra hop is not ideal, but I'm
not totally sure how expensive it is relative to the other operations. If we're not satisfied with write perf at v2,
maybe we'll try this.


#### Reads
Each node serves read traffic, and has an in-memory cache. My laptop serves ~90k cached reads/sec -- if we can get
even 5% of this we're easily in the clear.

The complexity comes in two places:
- Consistent hashing
- The chunk store

#### Consistent Hashing
Reads get routed to the appropriate node via a hashing scheme based on the chunk metadata in the state machine.

#### Chunk store
The chunk store downloads chunks from the blob store and then serves the values in these chunks.

