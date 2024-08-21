Url Shortener
=============
_**A very incomplete, unserious project.**_ 
A web service that implements URL shortening functionality akin to Bitly, Tinyurl etc. Designed to have a minimal footp
Deployed via fly.io because navigating AWS deployment for a personal project is bleh.


TODO
----
- [ ] Persistence [wip] -- right now, everything is in-memory, but we'll need to persist urls so that we don't wipe all created urls on restart
- [ ] Authentication -- there is no authentication, nor is there any user tracking. In order to implement billing, attribution, etc this is obviously necessary.
- [ ] Sharding -- right now, load-balancing is via fly defaults, which, once persistence is involved, will not take locality into account