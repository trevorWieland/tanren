# Migration Guide

## From pre-1.2 (aegis-era) to current

This is a **clean break** — no backward compatibility or runtime fallback.

### Secrets Path

The secrets directory moved from `~/.aegis/` to XDG-compliant `~/.config/tanren/`.

```bash
# Move your secrets
mkdir -p ~/.config/tanren
cp ~/.aegis/secrets.env ~/.config/tanren/secrets.env
cp -r ~/.aegis/secrets.d ~/.config/tanren/secrets.d 2>/dev/null

# Verify
tanren secret list

# Clean up old directory (once confirmed)
rm -rf ~/.aegis
```

If you use `XDG_CONFIG_HOME`, secrets go to `$XDG_CONFIG_HOME/tanren/` instead.

### IPC Directory

`WM_IPC_DIR` no longer has a default value. You **must** set it explicitly:

```bash
export WM_IPC_DIR=/path/to/your/ipc/directory
```

### Scripts and References

Update any scripts that reference:
- `~/.aegis/secrets.env` → `~/.config/tanren/secrets.env`
- `~/.aegis/secrets.d/` → `~/.config/tanren/secrets.d/`
- Default `WM_IPC_DIR` path → Set `WM_IPC_DIR` explicitly

### Checklist

- [ ] Move `~/.aegis/` contents to `~/.config/tanren/`
- [ ] Set `WM_IPC_DIR` environment variable explicitly
- [ ] Update any wrapper scripts referencing old paths
- [ ] Run `tanren secret list` to verify secrets are found
- [ ] Run `tanren env check` to verify env validation works
