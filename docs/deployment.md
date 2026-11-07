# Deployment Guide

How to run the service in production.

The service speaks plain HTTP on localhost and expects a reverse proxy on the
same host that terminates TLS.

## Configuration

Run the binary with these environment variables (or the flags of
`praxis-website --help`), ideally as a systemd service with `DynamicUser=`
and `StateDirectory=`.

| Variable                | Default                  | Purpose                           |
|-------------------------|--------------------------|-----------------------------------|
| `PRAXIS_BASE_URL`       | required                 | Public URL for canonical links    |
| `PRAXIS_BIND`           | `127.0.0.1:8080`         | Listen address                    |
| `STATE_DIRECTORY`       | required, set by systemd | Where the state files live        |
| `PRAXIS_MATOMO_URL`     | off                      | Matomo installation for analytics |
| `PRAXIS_MATOMO_SITE_ID` | off                      | Matomo site ID                    |
| `RUST_LOG`              | `info`                   | Log filter, e.g., `debug`         |

The service sends its own security headers and precompressed responses, so
the proxy needs no extra headers or compression settings.

## State

The state directory (`/var/lib/private/praxis-website` with `DynamicUser=`)
contains small JSON files. Back them up; all are written atomically.

- `statistik.json`: page views per day and page, kept for a year. If the file
  can't be read, the service starts over with empty statistics and keeps the
  old file as `statistik.json.unreadable`.

Days in the statistics and times in the login area use the time zone of the
system, so set `time.timeZone = "Europe/Berlin";` on NixOS.

## Operation

```sh
journalctl -u praxis-website -f            # Follow the log
journalctl -u praxis-website -p warning    # Only problems
journalctl -u praxis-website -g 'access statistics'  # Daily summaries
```

The log has one line per request (method, path, status, duration), and after
each day a summary of the access statistics. Client addresses are never
logged or stored.
