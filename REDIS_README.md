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
redis-cli -a your_redis_password info

# Sentinel status
redis-cli -p 26379 sentinel master mymaster
```

### Application Configuration

Set the Redis URL in your environment:
```bash
export REDIS_URL="redis://:<your_redis_password>@127.0.0.1:6379"
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

## Application Usage

[Application usage documentation to be added]

## Contributing

[Contributing guidelines to be added]

## License

[License information to be added] 