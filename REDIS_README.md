# Price Publisher

A real-time cryptocurrency price publisher that aggregates and publishes price data from multiple exchanges.

## System Requirements

- Rust (latest stable version)
- Redis Server (7.0+)
- Linux/Unix environment

## Redis Setup and Configuration

The application requires a Redis instance for price data storage and distribution. We've configured a fault-tolerant Redis setup using Redis Sentinel.

### Redis Configuration Overview

The Redis setup includes:
- Main Redis server for data storage
- Redis Sentinel for high availability
- Automatic restart capabilities
- Data persistence
- Memory management

### Redis Server Details

#### Basic Configuration
- Port: 6379
- Bind Address: 127.0.0.1
- Memory Limit: 256MB
- Memory Policy: LRU (Least Recently Used)

#### Persistence Configuration
- RDB Snapshots:
  - Every 900 seconds if at least 1 key changed
  - Every 300 seconds if at least 10 keys changed
  - Every 60 seconds if at least 10000 keys changed
- Append-Only File (AOF):
  - Enabled with 1-second fsync
  - Auto-rewrite at 100% growth
  - Minimum size for rewrite: 64MB

#### High Availability
Redis Sentinel is configured for automatic failover:
- Sentinel Port: 26379
- Down Detection: 5000ms
- Failover Timeout: 60000ms

### Installation and Setup

1. Install Redis and Sentinel:
```bash
sudo apt update && sudo apt install -y redis-server redis-sentinel
```

2. Configuration files:
- Redis: `/etc/redis/redis.conf`
- Sentinel: `/etc/redis/sentinel.conf`
- Systemd overrides: 
  - `/etc/systemd/system/redis-server.service.d/override.conf`
  - `/etc/systemd/system/redis-sentinel.service.d/override.conf`

3. Service Management:
```bash
# Start services
sudo systemctl start redis-server redis-sentinel

# Enable on boot
sudo systemctl enable redis-server redis-sentinel

# Check status
sudo systemctl status redis-server redis-sentinel
```

### Redis CLI Management Commands

General info dump:
```base
echo "=== Redis Server Info ===" && \
redis-cli info server | grep -E "redis_version|uptime|connected|used_memory_human" && \
echo -e "\n=== Price Keys ===" && \
redis-cli keys "price:*" && \
echo -e "\n=== Memory Usage ===" && \
redis-cli info memory | grep -E "used_memory_human|used_memory_peak_human|used_memory_rss_human"
```

#### 1. Basic Server Information
```bash
# Test if Redis is responding
redis-cli ping

# General server info
redis-cli info

# Server stats only
redis-cli info server

# Memory usage information
redis-cli info memory

# Client connection information
redis-cli info clients

# Command statistics
redis-cli info stats
```

#### 2. Real-time Monitoring
```bash
# Monitor all Redis commands in real-time
redis-cli monitor

# Watch live stats (CPU, memory, etc.)
redis-cli --stat

# Watch specific keys (e.g., all price keys)
redis-cli --scan --pattern "price:*"
```

#### 3. Key Management
```bash
# Count total number of keys
redis-cli dbsize

# List all keys matching a pattern
redis-cli keys "price:*"

# Get the type of a key
redis-cli type "price:BTCUSDT"

# Get time to live for a key
redis-cli ttl "price:BTCUSDT"
```

#### 4. Memory Analysis
```bash
# Get memory usage for a specific key
redis-cli memory usage "price:BTCUSDT"

# Find biggest keys in the database
redis-cli --bigkeys

# Get memory doctor report
redis-cli memory doctor
```

#### 5. Performance Analysis
```bash
# Get slow log entries
redis-cli slowlog get

# Get slow log length
redis-cli slowlog len

# Reset slow log
redis-cli slowlog reset
```

### Security

The Redis instance is password-protected. To change the default password:
```bash
redis-cli
> AUTH your_redis_password
> CONFIG SET requirepass "your_new_secure_password"
> CONFIG REWRITE
```

### Monitoring

Monitor Redis health:
```bash
# Redis server status
redis-cli info

# Sentinel status
redis-cli -p 26379 sentinel master mymaster

# Quick status check (memory, clients, etc.)
redis-cli info | grep -E "redis_version|uptime|connected|used_memory_human"
```

### Application Configuration

Set the Redis URL in your environment:
```bash
export REDIS_URL="redis://127.0.0.1:6379"
```

### Troubleshooting

1. If Redis fails to start:
   - Check logs: `sudo journalctl -u redis-server`
   - Verify config: `redis-cli --stat`
   - Check permissions: `ls -l /var/lib/redis`

2. If Sentinel fails:
   - Check logs: `sudo journalctl -u redis-sentinel`
   - Verify Sentinel status: `redis-cli -p 26379 info`

3. Common issues:
   - Permission denied: Ensure Redis user has proper permissions
   - Can't connect: Check if Redis is bound to correct interface
   - Memory errors: Verify available system memory and Redis limits

4. Performance issues:
   - Check slow log: `redis-cli slowlog get`
   - Monitor command latency: `redis-cli --latency`
   - Check memory fragmentation: `redis-cli info memory | grep fragmentation`

### Best Practices

1. Regular Monitoring:
   - Check memory usage regularly
   - Monitor slow operations
   - Watch for connection spikes

2. Maintenance:
   - Regularly check logs for errors
   - Monitor persistence status
   - Review and clean up old/unused keys

3. Performance:
   - Use pattern matching sparingly
   - Monitor slow operations
   - Keep an eye on memory fragmentation

## Application Usage

[Application usage documentation to be added]

## Contributing

[Contributing guidelines to be added]

## License

[License information to be added] 