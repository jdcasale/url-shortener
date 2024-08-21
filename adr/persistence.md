Persistence \[Draft\]
===========

Right now, the service that drops all of its state on the floor on restart -- this is obviously not desirable, as users
will expect some sort of formal contract for the lifetimes of their shortened links, and "idk my bff jill" is not an
acceptable contract.

We need a mechanism for persisting link state beyond restart. This can be broken down into several problems:

### Transactional Storage
Every time we create a link, we want to store a persistent record. We could offload this to a cloud service which is 
responsible for fault tolerance, redundancy, etc, but this kind of side-steps the core engineering exercise here. 
(cloud services are expensive, and we're trying to create a minimally complex, minimally expensive deployment)  

Solution? We'll use raft and rocksdb to maintain a strongly-consistent shared kv store  

### Backups
Occa