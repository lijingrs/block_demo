# Current Issues

1. ​**Concurrency Handling**:
   
   - At 100 concurrent requests, the virtual machine (VM) can handle the load, but containers cannot. Additionally, as concurrency increases, it blocks calls to other interfaces.

2. ​**Database Connection Pool Issues**:
   
   - The database uses a connection pool, but there is a certain probability of connection acquisition timeouts.

3. ​**Performance Discrepancy Between VM and Container**:
   
   - There is a significant performance gap between deployments on VMs and containers. Under the same CPU conditions, containers are much slower.


