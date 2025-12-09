# High Availability Deployment Guide

This guide explains how to deploy Quadrant VMS in a high-availability (HA) configuration with multiple coordinator instances and stateless worker nodes.

## Architecture Overview

Quadrant VMS supports active-active HA deployments with the following components:

### Components

1. **Multiple Coordinator Instances**
   - Run in a cluster with leader election
   - Share state via PostgreSQL StateStore
   - Automatic failover on leader failure

2. **Stateless Worker Nodes**
   - admin-gateway, stream-node, recorder-node, ai-service
   - Persist state to StateStore via HTTP API
   - Automatic recovery after restart
   - Orphan cleanup on startup

3. **PostgreSQL Database**
   - Stores leases and resource state
   - Shared by all coordinators
   - Should be deployed with HA (replication)

## State Management

### StateStore

The StateStore provides persistent state management for all resources:

- **Streams**: Video stream ingestion state
- **Recordings**: Recording job state
- **AI Tasks**: AI processing task state

Each resource tracks:
- Configuration
- Current state (Pending, Running, Error, etc.)
- Lease ID (for distributed locking)
- Node ID (which worker is handling it)
- Error messages
- Timestamps

### State Persistence Flow

1. **Worker Startup**:
   ```
   admin-gateway starts
   → Calls bootstrap() to restore state from StateStore
   → Resumes managing previously active resources
   → Calls cleanup_orphans() to clean up crashed resources
   ```

2. **Resource Creation**:
   ```
   User requests stream start
   → admin-gateway acquires lease from coordinator
   → Creates in-memory StreamInfo
   → Persists to StateStore
   → Starts worker process
   ```

3. **Resource Updates**:
   ```
   State changes (Running → Error)
   → Update in-memory state
   → Persist to StateStore
   → Continue operation
   ```

4. **Lease Renewal**:
   ```
   Every TTL/2 seconds:
   → Health check worker
   → Renew lease with coordinator
   → On failure: mark Error state and persist
   ```

5. **Worker Restart**:
   ```
   admin-gateway restarts
   → bootstrap() loads all resources for this node_id
   → Restores in-memory maps
   → cleanup_orphans() removes stale state
   → Resume lease renewals
   ```

## Deployment Configurations

### Single Coordinator (Development)

```yaml
services:
  coordinator:
    image: quadrant-vms/coordinator:latest
    environment:
      - STORE_TYPE=postgres
      - DATABASE_URL=postgresql://postgres:password@db:5432/quadrant_vms
    ports:
      - "8080:8080"

  db:
    image: postgres:17
    environment:
      - POSTGRES_PASSWORD=password
      - POSTGRES_DB=quadrant_vms
    volumes:
      - postgres_data:/var/lib/postgresql/data
```

### Multi-Coordinator HA (Production)

```yaml
services:
  coordinator-1:
    image: quadrant-vms/coordinator:latest
    environment:
      - NODE_ID=coordinator-1
      - CLUSTER_ENABLED=true
      - STORE_TYPE=postgres
      - DATABASE_URL=postgresql://postgres:password@db:5432/quadrant_vms
      - PEER_ADDRS=http://coordinator-2:8080,http://coordinator-3:8080
      - ELECTION_TIMEOUT_MS=5000
      - HEARTBEAT_INTERVAL_MS=1000
    ports:
      - "8081:8080"

  coordinator-2:
    image: quadrant-vms/coordinator:latest
    environment:
      - NODE_ID=coordinator-2
      - CLUSTER_ENABLED=true
      - STORE_TYPE=postgres
      - DATABASE_URL=postgresql://postgres:password@db:5432/quadrant_vms
      - PEER_ADDRS=http://coordinator-1:8080,http://coordinator-3:8080
      - ELECTION_TIMEOUT_MS=5000
      - HEARTBEAT_INTERVAL_MS=1000
    ports:
      - "8082:8080"

  coordinator-3:
    image: quadrant-vms/coordinator:latest
    environment:
      - NODE_ID=coordinator-3
      - CLUSTER_ENABLED=true
      - STORE_TYPE=postgres
      - DATABASE_URL=postgresql://postgres:password@db:5432/quadrant_vms
      - PEER_ADDRS=http://coordinator-1:8080,http://coordinator-2:8080
      - ELECTION_TIMEOUT_MS=5000
      - HEARTBEAT_INTERVAL_MS=1000
    ports:
      - "8083:8080"

  admin-gateway-1:
    image: quadrant-vms/admin-gateway:latest
    environment:
      - NODE_ID=admin-gateway-1
      - ENABLE_STATE_STORE=true
      - COORDINATOR_BASE_URL=http://coordinator-1:8080
      - ORPHAN_CLEANUP_INTERVAL_SECS=300
    depends_on:
      - coordinator-1

  admin-gateway-2:
    image: quadrant-vms/admin-gateway:latest
    environment:
      - NODE_ID=admin-gateway-2
      - ENABLE_STATE_STORE=true
      - COORDINATOR_BASE_URL=http://coordinator-2:8080
      - ORPHAN_CLEANUP_INTERVAL_SECS=300
    depends_on:
      - coordinator-2

  db:
    image: postgres:17
    environment:
      - POSTGRES_PASSWORD=password
      - POSTGRES_DB=quadrant_vms
    volumes:
      - postgres_data:/var/lib/postgresql/data
```

## Configuration

### Environment Variables

#### Coordinator

- `NODE_ID` - Unique identifier for this coordinator instance
- `CLUSTER_ENABLED` - Enable clustering (true/false)
- `STORE_TYPE` - Lease store type (memory/postgres)
- `DATABASE_URL` - PostgreSQL connection string
- `PEER_ADDRS` - Comma-separated list of peer coordinator URLs
- `ELECTION_TIMEOUT_MS` - Raft election timeout (default: 5000)
- `HEARTBEAT_INTERVAL_MS` - Raft heartbeat interval (default: 1000)

#### Worker Nodes (admin-gateway)

- `NODE_ID` - Unique identifier for this worker instance
- `ENABLE_STATE_STORE` - Enable StateStore integration (true/false)
- `COORDINATOR_BASE_URL` - Coordinator HTTP endpoint
- `ORPHAN_CLEANUP_INTERVAL_SECS` - How often to cleanup orphans (default: 300)

## Operational Procedures

### Initial Deployment

1. **Deploy PostgreSQL database**:
   ```bash
   # Ensure PostgreSQL is running and accessible
   psql -h localhost -U postgres -c "CREATE DATABASE quadrant_vms;"
   ```

2. **Run database migrations**:
   ```bash
   cd crates/coordinator
   DATABASE_URL="postgresql://postgres:password@localhost:5432/quadrant_vms" \
     cargo sqlx migrate run
   ```

3. **Start coordinators**:
   ```bash
   # All coordinators will participate in leader election
   docker-compose up -d coordinator-1 coordinator-2 coordinator-3
   ```

4. **Verify cluster health**:
   ```bash
   # Check each coordinator's status
   curl http://localhost:8081/v1/cluster/status
   curl http://localhost:8082/v1/cluster/status
   curl http://localhost:8083/v1/cluster/status

   # One should be leader, others followers
   ```

5. **Start worker nodes**:
   ```bash
   docker-compose up -d admin-gateway-1 admin-gateway-2
   ```

### Adding a New Coordinator

1. **Update peer list** on existing coordinators:
   ```bash
   # Add new peer to PEER_ADDRS environment variable
   PEER_ADDRS=http://coordinator-1:8080,http://coordinator-2:8080,http://coordinator-3:8080,http://coordinator-4:8080
   ```

2. **Start new coordinator**:
   ```bash
   docker-compose up -d coordinator-4
   ```

3. **Verify integration**:
   ```bash
   curl http://localhost:8084/v1/cluster/status
   ```

### Adding a New Worker Node

1. **Configure StateStore connection**:
   ```yaml
   environment:
     - NODE_ID=admin-gateway-3  # Must be unique
     - ENABLE_STATE_STORE=true
     - COORDINATOR_BASE_URL=http://coordinator-1:8080
   ```

2. **Start worker**:
   ```bash
   docker-compose up -d admin-gateway-3
   ```

3. **Worker will automatically**:
   - Connect to StateStore
   - Restore any previous state for its node_id
   - Start lease renewal for active resources

### Coordinator Failover

When a coordinator leader fails:

1. **Automatic detection**:
   - Followers detect missing heartbeats
   - Election timeout triggers new election
   - New leader is elected (typically within 5-10 seconds)

2. **State preservation**:
   - All state is in PostgreSQL StateStore
   - No data loss occurs
   - Workers continue operating
   - Lease renewals may temporarily fail, but will retry

3. **Worker adaptation**:
   - Workers retry lease renewals with exponential backoff
   - After 3 failures, mark resource as Error state
   - Admin can manually recover or restart workers

### Worker Node Failure

When a worker node crashes:

1. **Lease expiration**:
   - Leases expire after TTL seconds (default: 30)
   - Coordinator marks lease as released

2. **Orphan state**:
   - State remains in StateStore with last known state
   - State is marked as "orphaned" (non-active with lease_id)

3. **Recovery on restart**:
   ```
   Worker restarts
   → bootstrap() loads previous state
   → cleanup_orphans() deletes orphaned resources
   → Worker is clean and ready for new work
   ```

### State Migration

Use the `state-migrate` tool for maintenance:

```bash
# Check current schema version
cargo run --bin state-migrate -- --database-url $DB_URL check

# List orphaned resources
cargo run --bin state-migrate -- --database-url $DB_URL list-orphans

# Clean up orphans (dry run first)
cargo run --bin state-migrate -- --database-url $DB_URL cleanup-orphans --dry-run
cargo run --bin state-migrate -- --database-url $DB_URL cleanup-orphans

# Export state for backup
cargo run --bin state-migrate -- --database-url $DB_URL export backup.json --pretty

# Import state (skip existing)
cargo run --bin state-migrate -- --database-url $DB_URL import backup.json --skip-existing

# Database maintenance
cargo run --bin state-migrate -- --database-url $DB_URL vacuum

# View statistics
cargo run --bin state-migrate -- --database-url $DB_URL stats
```

## Monitoring

### Key Metrics

1. **Coordinator Metrics** (exposed on `/metrics`):
   - `coordinator_cluster_role` - Current role (0=follower, 1=leader)
   - `coordinator_cluster_term` - Current election term
   - `coordinator_lease_count` - Active leases
   - `coordinator_state_store_operations` - StateStore API calls

2. **Worker Metrics**:
   - `admin_gateway_active_streams` - Current active streams
   - `admin_gateway_active_recordings` - Current active recordings
   - `admin_gateway_orphan_cleanup_runs` - Orphan cleanup executions
   - `admin_gateway_lease_renewal_failures` - Failed lease renewals

### Health Checks

```bash
# Coordinator health
curl http://localhost:8080/healthz  # Should return "ok"
curl http://localhost:8080/readyz   # Should return 200

# Worker health
curl http://localhost:3000/healthz
curl http://localhost:3000/readyz

# StateStore health (via coordinator)
curl http://localhost:8080/v1/state/health
```

### Logs

Key log messages to monitor:

```
# Coordinator
INFO coordinator::cluster: elected as leader term=5
WARN coordinator::cluster: leader election timeout, starting new election
INFO coordinator::pg_state_store: state store operation completed

# Worker
INFO admin_gateway::state: state restored from StateStore
WARN admin_gateway::state: found orphaned stream stream_id=...
INFO admin_gateway::state: orphan cleanup completed cleaned_streams=2
ERROR admin_gateway::state: lease renewal failed after 3 retries
```

## Troubleshooting

### Issue: Split Brain (Multiple Leaders)

**Symptoms**: Multiple coordinators claim to be leader

**Solution**:
1. Check network connectivity between coordinators
2. Ensure PEER_ADDRS is correctly configured
3. Restart coordinators one at a time
4. Check for clock skew between nodes

### Issue: Orphaned Resources Not Cleaning Up

**Symptoms**: StateStore fills with Error state resources

**Solution**:
1. Verify ENABLE_STATE_STORE=true on workers
2. Check ORPHAN_CLEANUP_INTERVAL_SECS setting
3. Manually clean up: `state-migrate cleanup-orphans`
4. Check worker logs for cleanup errors

### Issue: Worker Can't Restore State

**Symptoms**: Worker starts but doesn't resume streams

**Solution**:
1. Verify DATABASE_URL is accessible from worker
2. Check StateStore API is responding: `curl http://coordinator:8080/v1/state/health`
3. Verify NODE_ID matches previous runs
4. Check logs for bootstrap errors

### Issue: Lease Renewal Failures

**Symptoms**: Resources marked as Error due to lease renewal failure

**Solution**:
1. Check network connectivity to coordinator
2. Verify coordinator is healthy and responding
3. Check coordinator leader status
4. Increase TTL if network is unreliable
5. Check worker health check is passing

## Best Practices

1. **Always enable StateStore in production**:
   ```
   ENABLE_STATE_STORE=true
   ```

2. **Use unique NODE_ID for each worker**:
   ```
   NODE_ID=admin-gateway-${HOSTNAME}
   ```

3. **Run at least 3 coordinators** for HA (odd number for Raft quorum)

4. **Deploy PostgreSQL with replication** (Patroni, Stolon, etc.)

5. **Monitor orphan cleanup metrics** and tune cleanup interval

6. **Backup state regularly**:
   ```bash
   # Daily backup
   state-migrate export /backup/state-$(date +%Y%m%d).json --pretty
   ```

7. **Test failover scenarios**:
   - Kill coordinator leader
   - Kill worker nodes
   - Network partition
   - Database connection loss

8. **Set appropriate TTLs**:
   - Default 30s for stable networks
   - 60s+ for unreliable networks
   - Short TTLs = faster failure detection but more network traffic

## Security Considerations

1. **Database Access**:
   - Use strong passwords
   - Enable TLS for PostgreSQL connections
   - Restrict database access to coordinator nodes only

2. **Inter-Service Communication**:
   - Use TLS for coordinator-worker communication
   - Implement authentication tokens
   - Restrict network access between components

3. **StateStore API**:
   - Consider adding authentication middleware
   - Rate limit StateStore endpoints
   - Log all state modifications for audit

## Performance Tuning

1. **Database Connection Pool**:
   ```rust
   // coordinator/src/main.rs
   PgPoolOptions::new()
       .max_connections(50)  // Tune based on load
       .acquire_timeout(Duration::from_secs(5))
       .connect(&database_url)
   ```

2. **Lease TTL**:
   - Lower TTL = faster failure detection, more traffic
   - Higher TTL = less traffic, slower failure detection
   - Recommended: 30-60 seconds

3. **Orphan Cleanup Interval**:
   - Too frequent = unnecessary database queries
   - Too infrequent = orphans accumulate
   - Recommended: 300 seconds (5 minutes)

4. **Database Indexes**:
   - Ensure indexes exist on node_id, lease_id, state columns
   - Run VACUUM ANALYZE regularly
   - Monitor query performance with pg_stat_statements

## Future Enhancements

- [ ] State replication across coordinators for read scalability
- [ ] Distributed tracing with OpenTelemetry
- [ ] State store caching layer (Redis)
- [ ] Automatic orphan cleanup on lease expiration
- [ ] Worker registration and discovery
- [ ] Rolling upgrades support
- [ ] Multi-region deployment guide
