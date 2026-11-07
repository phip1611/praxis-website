# Deployment Guide

How to run the service in production.

The service speaks plain HTTP on localhost and expects a reverse proxy on the
same host that terminates TLS. Logins are limited per client (5 failures per
15 minutes, if the proxy reports the client address), per account (20), and
globally (600 password checks per minute).

## Configuration

Run the binary with these environment variables (or the flags of
`praxis-website --help`), ideally as a systemd service with `DynamicUser=`,
`StateDirectory=`, and `LoadCredential=accounts:<file>`.

| Variable                  | Default                           | Purpose                                                    |
|---------------------------|-----------------------------------|------------------------------------------------------------|
| `PRAXIS_BASE_URL`         | required                          | Public URL for canonical links, cookies, and origin checks |
| `PRAXIS_BIND`             | `127.0.0.1:8080`                  | Listen address                                             |
| `STATE_DIRECTORY`         | required, set by systemd          | Where the state files live                                 |
| `PRAXIS_ACCOUNTS_FILE`    | required                          | The accounts file                                          |
| `PRAXIS_CLIENT_IP_HEADER` | off                               | Proxy header with the client address                       |
| `PRAXIS_MATOMO_URL`       | off                               | Matomo installation for analytics                          |
| `PRAXIS_MATOMO_SITE_ID`   | off                               | Matomo site ID                                             |
| `RUST_LOG`                | `info`                            | Log filter, e.g., `debug`                                  |

Only set `PRAXIS_CLIENT_IP_HEADER` if the proxy overwrites that header, as
nginx does with `proxy_set_header X-Real-IP $remote_addr`. Otherwise,
clients could set it themselves and get other clients blocked.

The service sends its own security headers and precompressed responses, so
the proxy needs no extra headers or compression settings.

## Accounts

The accounts file has one `username:hash` line per account. The hash is
created with `praxis-website hash-password <username>`. Changes take effect on
the next start of the service, which logs out everyone. Leaked hashes are
fine: Argon2id and the 15-character minimum make them practically
uncrackable. The service can't add accounts, as it can write its state
directory only.

Users change their password in the login area. The new hash is stored in the
state directory and wins over the configured one until the configured hash
changes. So to reset a forgotten password, configure a new hash. The service
can't send e-mails, the login area says so.

## State

The state directory (`/var/lib/private/praxis-website` with `DynamicUser=`)
contains small JSON files. Back them up; all are written atomically.

- `aktuelle-informationen.json`: the news on the start page. Without it, the
  service shows the content as of the migration from WordPress.
- `statistik.json`: page views per day and page, kept for a year. If the file
  can't be read, the service starts over with empty statistics and keeps the
  old file as `statistik.json.unreadable`.
- `passwoerter.json`: hashes of passwords changed in the login area.

Days in the statistics and times in the login area use the time zone of the
system, so set `time.timeZone = "Europe/Berlin";` on NixOS.

## Operation

```sh
journalctl -u praxis-website -f            # Follow the log
journalctl -u praxis-website -p warning    # Only problems
journalctl -u praxis-website -g 'access statistics'  # Daily summaries
```

The log has one line per request (method, path, status, duration), logins,
edits, and after each day a summary of the access statistics. Client addresses
are never logged or stored. After failed logins, the service keeps a pseudonym
of the client address in memory, for at most 16 minutes after the last failure.
The detailed statistics are at `/admin/statistik/` after logging in.
