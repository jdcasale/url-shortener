Redundancy \[Draft\]
==========

In practice, we will want to deploy multiple copies of our service so that when one instance inevitably has downtime
(failures, upgrades, etc), users do not encounter service outages.



Questions
---------
### Data replication
<u>_**Should we replicate data across nodes?**_</u>

Right now, if a node goes down, the data that it services does not exist anywhere else. 
With an in-memory cache, this means that the data is lost. We could address this by keeping a replication factor above
1 and synchronizing data across deployed nodes.

Synchronization and repair-on-failure are nontrivial 


- Sharding
  - We could rely on the reverse proxy to route traffic between instance based on 