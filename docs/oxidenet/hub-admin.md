# Hub Admin

Install hub defaults:

```bash
oxidebbs-server net oxidenet install-hub --host blackboard.example.net
```

Review applications:

```bash
oxidebbs-server net oxidenet applications list
oxidebbs-server net oxidenet applications show <application-id>
oxidebbs-server net oxidenet applications approve <application-id> \
  --package-dir ./packages/<application-id>
oxidebbs-server net oxidenet applications request-info <application-id> --notes "Need DNS name"
oxidebbs-server net oxidenet applications hold <application-id>
oxidebbs-server net oxidenet applications reject <application-id>
```

Operate nodes:

```bash
oxidebbs-server net oxidenet nodes list
oxidebbs-server net oxidenet nodes suspend 42:1/100
oxidebbs-server net oxidenet nodes activate 42:1/100
oxidebbs-server net oxidenet nodes rotate-password 42:1/100
oxidebbs-server net oxidenet tokens issue 42:1/100
```

Publish a nodelist:

```bash
oxidebbs-server net oxidenet nodelist --output ./public/oxidenet.nodelist
```

The local sysop TUI exposes the same registry state on the OxideNet screen.
