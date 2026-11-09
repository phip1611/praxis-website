# VM test of the NixOS module: the hardened service starts, serves pages,
# accepts a login and an edit, and keeps the edit across restarts. A second
# machine runs a test instance that search engines must not index.

let
  # The password is "correct horse battery staple".
  account = "$argon2id$v=19$m=19456,t=2,p=1$uNOFUfQlmLHNoWgYsJDyYw$2wlK3UWEKzL3jn8L4o0AlqLEBW3exlpbu+j1XIZiSMQ";
in
{
  name = "praxis-website";

  nodes.machine =
    { pkgs, ... }:
    {
      imports = [ ../module.nix ];

      services.praxis-website = {
        enable = true;
        baseUrl = "https://praxis.test";
        port = 8028;
        accounts.e2e = account;
        clientIpHeader = "X-Real-IP";
      };

      time.timeZone = "Asia/Tokyo";

      environment.systemPackages = [ pkgs.curl ];
    };

  nodes.hidden =
    { pkgs, ... }:
    {
      imports = [ ../module.nix ];

      services.praxis-website = {
        enable = true;
        baseUrl = "https://test.praxis.test";
        port = 8028;
        accounts.e2e = account;
        noindex = true;
      };

      environment.systemPackages = [ pkgs.curl ];
    };

  testScript = ''
    import re

    def search(pattern, text):
        match = re.search(pattern, text, re.IGNORECASE)
        assert match is not None, text
        return match.group(1)

    base = "http://127.0.0.1:8028"
    origin = "-H 'Origin: https://praxis.test'"

    # A short timeout, as a broken sandbox makes the service restart forever.
    machine.wait_for_unit("praxis-website.service", timeout=60)
    machine.wait_for_open_port(8028)

    with subtest("public pages are served compressed"):
        assert "Aktuelle Informationen" in machine.succeed(f"curl -sf {base}/")
        assert "Stadtbadstraße 1b" in machine.succeed(f"curl -sf {base}/kontakt/")
        headers = machine.succeed(f"curl -sf -o /dev/null -D - -H 'Accept-Encoding: br' {base}/")
        assert "content-encoding: br" in headers.lower(), headers

    with subtest("only the test instance asks search engines to skip it"):
        hidden.wait_for_unit("praxis-website.service", timeout=60)
        hidden.wait_for_open_port(8028)
        headers = hidden.succeed(f"curl -sf -o /dev/null -D - {base}/")
        assert "x-robots-tag: noindex, nofollow" in headers.lower(), headers
        headers = machine.succeed(f"curl -sf -o /dev/null -D - {base}/")
        assert "x-robots-tag" not in headers.lower(), headers

    with subtest("the hardening is effective"):
        # Fails if the exposure level is above 1.0 (currently 0.9).
        score = machine.succeed("systemd-analyze security --threshold=10 praxis-website.service")
        print(score.strip().splitlines()[-1])
        # The state directory belongs to the dynamic user only.
        machine.succeed("test \"$(stat -c %a /var/lib/private/praxis-website)\" = 700")

    with subtest("login and edit work"):
        # The session cookie is `Secure`, which curl doesn't send over plain
        # HTTP from its cookie jar, so the header is passed by hand.
        headers = machine.succeed(
            f"curl -s -o /dev/null -D - {origin} "
            "--data-urlencode username=e2e "
            "--data-urlencode 'password=correct horse battery staple' "
            f"{base}/admin/login/"
        )
        cookie = search(r"set-cookie: ([^;]+)", headers)
        editor = machine.succeed(f"curl -sf -H 'Cookie: {cookie}' {base}/admin/")
        # The times are in the time zone of the system.
        assert "07.11.2026 um 17:31 Uhr" in editor, editor
        csrf = search(r'name="csrf" value="([^"]+)"', editor)
        status = machine.succeed(
            f"curl -s -o /dev/null -w '%{{http_code}}' {origin} -H 'Cookie: {cookie}' "
            f"--data-urlencode csrf={csrf} --data-urlencode revision=1 "
            "--data-urlencode 'html=<p>Aus dem VM-Test</p>' "
            f"{base}/admin/"
        )
        assert status == "303", status
        assert "Aus dem VM-Test" in machine.succeed(f"curl -sf {base}/")

    with subtest("a guessing client is blocked, others can still log in"):
        def login(client, password):
            return machine.succeed(
                f"curl -s -o /dev/null -w '%{{http_code}}' {origin} "
                f"-H 'X-Real-IP: {client}' --data-urlencode username=e2e "
                f"--data-urlencode 'password={password}' {base}/admin/login/"
            )
        for _ in range(5):
            assert login("198.51.100.66", "wrong") == "401"
        status = login("198.51.100.66", "correct horse battery staple")
        assert status == "429", status
        status = login("192.0.2.10", "correct horse battery staple")
        assert status == "303", status

    with subtest("the edit survives a restart"):
        machine.systemctl("restart praxis-website.service")
        machine.wait_for_open_port(8028)
        assert "Aus dem VM-Test" in machine.succeed(f"curl -sf {base}/")
        machine.succeed("test -s /var/lib/private/praxis-website/statistik.json")

    with subtest("the journal has priorities and no errors"):
        assert "listening on" in machine.succeed("journalctl -u praxis-website -p info -o cat")
        machine.fail("journalctl -u praxis-website -p err -o cat | grep .")
  '';
}
