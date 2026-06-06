# Setup Member

After approval, the hub admin gives the member sysop a config package and the
one-time plaintext session password.

Import the package:

```bash
oxidebbs-server net oxidenet package import ./oxidenet-package
```

Verify state:

```bash
oxidebbs-server net oxidenet status
oxidebbs-server net status oxidenet
oxidebbs-server net areas list --network oxidenet
```

Poll the hub:

```bash
oxidebbs-server net poll oxidenet-hub
```

If the hub rotates the password, import a regenerated package or update the
`oxidenet-hub` link with the newly displayed plaintext secret.
