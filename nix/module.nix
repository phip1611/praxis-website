# NixOS module that runs the website as hardened systemd service.
#
# The service listens on the loopback interface only. A reverse proxy on the
# same host, e.g., nginx, terminates TLS and forwards to it.

{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.praxis-website;

  # The accounts file in the format of `praxis-website hash-password`.
  accountsFile =
    if cfg.accountsFile != null then
      cfg.accountsFile
    else
      pkgs.writeText "praxis-website-accounts" (
        lib.concatLines (lib.mapAttrsToList (username: hash: "${username}:${hash}") cfg.accounts)
      );
in
{
  options.services.praxis-website = {
    enable = lib.mkEnableOption "the website of the general practice Dipl.-Med. Dirk Schuster";

    package = lib.mkOption {
      type = lib.types.package;
      description = ''
        The praxis-website package to use. The module of the flake defaults to
        the package of the flake.
      '';
    };

    baseUrl = lib.mkOption {
      type = lib.types.str;
      example = "https://praxis-dirkschuster.de";
      description = ''
        The public URL of the site, as seen by visitors. Used for canonical
        links, the sitemap, the session cookie, and the origin check of the
        login area.
      '';
    };

    port = lib.mkOption {
      type = lib.types.port;
      default = 8080;
      description = "The port on 127.0.0.1 to listen on.";
    };

    accounts = lib.mkOption {
      type = lib.types.attrsOf lib.types.str;
      default = { };
      example = {
        dirk = "$argon2id$v=19$m=19456,t=2,p=1$...";
      };
      description = ''
        The accounts of the login area: username and Argon2id hash, created
        with `praxis-website hash-password <username>`. The hashes end up in
        the world-readable Nix store, which is fine for the passwords of at
        least 15 characters that `hash-password` enforces. Use `accountsFile`
        to keep them out of the store.
      '';
    };

    accountsFile = lib.mkOption {
      # A string and not a path, so that the file doesn't end up in the
      # world-readable Nix store.
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "/run/secrets/praxis-website-accounts";
      description = ''
        Alternative to `accounts`: a file with one `username:hash` line per
        account, e.g., from a secret manager. It's passed to the service as
        systemd credential, so it may be readable by root only.
      '';
    };

    clientIpHeader = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "X-Real-IP";
      description = ''
        The header in which the reverse proxy reports the client address.
        Enables the per-client limit of failed logins. Only set it if the
        proxy overwrites the header, as clients can send it themselves. nginx
        with `recommendedProxySettings` sets `X-Real-IP`.
      '';
    };

    matomo = lib.mkOption {
      type = lib.types.nullOr (
        lib.types.submodule {
          options = {
            url = lib.mkOption {
              type = lib.types.str;
              example = "https://matomo.example.com/";
              description = "The URL of the Matomo installation.";
            };
            siteId = lib.mkOption {
              type = lib.types.ints.positive;
              description = "The ID of the site in Matomo.";
            };
          };
        }
      );
      default = null;
      description = "The cookie-less Matomo web analytics, or `null` for none.";
    };

    noindex = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Whether to keep the site out of search results, e.g., for a test
        instance that is reachable but not public. All responses then carry
        `X-Robots-Tag: noindex, nofollow`. Canonical links still point to
        `baseUrl`.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = (cfg.accounts != { }) != (cfg.accountsFile != null);
        message = "services.praxis-website: set either `accounts` or `accountsFile`.";
      }
    ];

    systemd.services.praxis-website = {
      description = "Website of the general practice Dipl.-Med. Dirk Schuster";
      wantedBy = [ "multi-user.target" ];
      after = [ "network.target" ];

      environment = {
        PRAXIS_BIND = "127.0.0.1:${toString cfg.port}";
        PRAXIS_BASE_URL = cfg.baseUrl;
        PRAXIS_ACCOUNTS_FILE = "%d/accounts";
      }
      // lib.optionalAttrs (cfg.clientIpHeader != null) {
        PRAXIS_CLIENT_IP_HEADER = cfg.clientIpHeader;
      }
      // lib.optionalAttrs (cfg.matomo != null) {
        PRAXIS_MATOMO_URL = cfg.matomo.url;
        PRAXIS_MATOMO_SITE_ID = toString cfg.matomo.siteId;
      }
      // lib.optionalAttrs cfg.noindex {
        PRAXIS_NOINDEX = "true";
      };

      serviceConfig = {
        Type = "exec";
        ExecStart = "${lib.getExe cfg.package} serve";
        Restart = "on-failure";
        RestartSec = "2s";
        # Above the service's own shutdown grace period of 10 seconds, so
        # that it saves the access statistics before systemd kills it.
        TimeoutStopSec = "30s";

        # The news and the access statistics: /var/lib/praxis-website.
        StateDirectory = "praxis-website";
        StateDirectoryMode = "0700";
        # Readable as `%d/accounts`, see `PRAXIS_ACCOUNTS_FILE`.
        LoadCredential = [ "accounts:${accountsFile}" ];

        # Hardening. `systemd-analyze security praxis-website` rates this below
        # 1.0 ("SAFE"); the VM test in nix/tests checks that.
        DynamicUser = true;
        UMask = "0077";
        CapabilityBoundingSet = "";
        AmbientCapabilities = "";
        NoNewPrivileges = true;
        PrivateTmp = true;
        PrivateDevices = true;
        PrivateUsers = true;
        PrivateIPC = true;
        DevicePolicy = "closed";
        ProtectSystem = "strict";
        ProtectHome = true;
        ProtectHostname = true;
        ProtectClock = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectKernelLogs = true;
        ProtectControlGroups = "strict";
        ProtectProc = "invisible";
        ProcSubset = "pid";
        # Only the service binary is executed, from the Nix store.
        NoExecPaths = [ "/" ];
        ExecPaths = [ "/nix/store" ];
        RestrictAddressFamilies = [
          "AF_INET"
          "AF_INET6"
        ];
        SocketBindAllow = [ "tcp:${toString cfg.port}" ];
        SocketBindDeny = [ "any" ];
        # Only the reverse proxy on this host may connect, and the service
        # itself doesn't connect anywhere.
        IPAddressDeny = "any";
        IPAddressAllow = "localhost";
        RestrictNamespaces = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        RemoveIPC = true;
        LockPersonality = true;
        MemoryDenyWriteExecute = true;
        SystemCallArchitectures = "native";
        SystemCallFilter = [
          "@system-service"
          "~@privileged"
          "~@resources"
        ];
        SystemCallErrorNumber = "EPERM";
        # The service needs about 20 MiB. The limits stop runaway growth.
        MemoryMax = "256M";
        TasksMax = 64;
      };
    };
  };
}
