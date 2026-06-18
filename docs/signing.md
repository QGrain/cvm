# Release Signing

cvm release assets are signed with a GPG key dedicated to cvm releases.
Do not use a personal primary key.

## Create a Release Key

```sh
gpg --full-generate-key
```

Recommended choices:

- Key type: RSA and RSA
- Key size: 4096
- Expiration: 1y or 2y
- Name: `cvm release signing`
- Email: an address you control
- Passphrase: a strong passphrase stored outside the repository

Find the key id:

```sh
gpg --list-secret-keys --keyid-format LONG
```

Export the public key into the repository:

```sh
mkdir -p assets/keys
gpg --armor --export <KEY_ID> > assets/keys/cvm-release-signing-key.asc
```

Export the private key for GitHub Actions secrets:

```sh
gpg --armor --export-secret-keys <KEY_ID> > /tmp/cvm-release-signing-private.asc
```

Do not commit the private key.

## GitHub Secrets

Configure these repository secrets:

```text
CVM_RELEASE_GPG_PRIVATE_KEY
CVM_RELEASE_GPG_PASSPHRASE
CVM_RELEASE_GPG_KEY_ID
```

Set `CVM_RELEASE_GPG_PRIVATE_KEY` to the full armored private key content from
`/tmp/cvm-release-signing-private.asc`. Set `CVM_RELEASE_GPG_KEY_ID` to the key
id shown by `gpg --list-secret-keys --keyid-format LONG`.

After configuring the secrets, delete the temporary private key export:

```sh
shred -u /tmp/cvm-release-signing-private.asc
```

If `shred` is unavailable, delete the file and ensure it is not backed up or
committed.
