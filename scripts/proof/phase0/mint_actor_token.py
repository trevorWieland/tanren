#!/usr/bin/env python3
"""Mint Ed25519-signed actor tokens for Phase 0 proof runs.

This utility intentionally centralizes token math so proof runs don't fail
because of manual `exp - iat` mistakes.
"""

from __future__ import annotations

import argparse
import base64
import json
import sys
import time
import uuid
from dataclasses import dataclass
from pathlib import Path

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey


@dataclass(frozen=True)
class _Claims:
    iss: str
    aud: str
    exp: int
    nbf: int
    iat: int
    jti: str
    org_id: str
    user_id: str


def _b64url(data: bytes) -> str:
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode("ascii")


def _load_private_key(path: Path) -> Ed25519PrivateKey:
    key_bytes = path.read_bytes()
    key = serialization.load_pem_private_key(key_bytes, password=None)
    if not isinstance(key, Ed25519PrivateKey):
        raise TypeError("private key is not Ed25519")
    return key


def _sign_jwt(private_key: Ed25519PrivateKey, claims: _Claims, kid: str) -> str:
    header = {"alg": "EdDSA", "typ": "JWT", "kid": kid}
    payload = {
        "iss": claims.iss,
        "aud": claims.aud,
        "exp": claims.exp,
        "nbf": claims.nbf,
        "iat": claims.iat,
        "jti": claims.jti,
        "org_id": claims.org_id,
        "user_id": claims.user_id,
    }

    header_segment = _b64url(json.dumps(header, separators=(",", ":")).encode("utf-8"))
    payload_segment = _b64url(json.dumps(payload, separators=(",", ":")).encode("utf-8"))
    signing_input = f"{header_segment}.{payload_segment}".encode()
    signature = private_key.sign(signing_input)
    signature_segment = _b64url(signature)
    return f"{header_segment}.{payload_segment}.{signature_segment}"


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Mint proof actor tokens")
    parser.add_argument("--private-key-pem", type=Path, required=True)
    parser.add_argument("--issuer", required=True)
    parser.add_argument("--audience", required=True)
    parser.add_argument("--org-id", required=True)
    parser.add_argument("--user-id", required=True)
    parser.add_argument(
        "--mode",
        required=True,
        choices=[
            "valid",
            "wrong_issuer",
            "wrong_audience",
            "expired",
            "ttl_over_max",
            "replay_reuse",
        ],
    )
    parser.add_argument("--requested-ttl", type=int, default=600)
    parser.add_argument("--max-ttl", type=int, default=900)
    parser.add_argument("--iat", type=int, default=None)
    parser.add_argument("--kid", default="phase0-proof")
    parser.add_argument("--jti", default=None)
    parser.add_argument("--token-only", action="store_true")
    return parser.parse_args()


def _main() -> int:
    args = _parse_args()

    now = int(time.time())
    iat = args.iat if args.iat is not None else now
    requested_ttl = args.requested_ttl

    iss = args.issuer
    aud = args.audience
    jti = args.jti or str(uuid.uuid4())

    if args.mode == "wrong_issuer":
        iss = f"{args.issuer}-wrong"
    elif args.mode == "wrong_audience":
        aud = f"{args.audience}-wrong"
    elif args.mode == "replay_reuse":
        jti = args.jti or "phase0-proof-replay-jti"

    if args.mode == "expired":
        exp = iat - 1
        nbf = iat - 300
    elif args.mode == "ttl_over_max":
        exp = iat + max(requested_ttl, args.max_ttl + 1)
        nbf = iat - 30
    else:
        exp = iat + requested_ttl
        nbf = iat - 30

    exp_minus_iat = exp - iat

    print(
        f"iat={iat} exp={exp} exp_minus_iat={exp_minus_iat} max_ttl={args.max_ttl}",
        file=sys.stderr,
    )

    ttl_violation = exp_minus_iat > args.max_ttl
    if ttl_violation:
        print(
            "warning: exp_minus_iat exceeds actor_token_max_ttl_secs",
            file=sys.stderr,
        )

    # For non-ttl_over_max modes, fail closed on TTL mistakes.
    if ttl_violation and args.mode != "ttl_over_max":
        print(
            "error: refusing to mint token because exp_minus_iat exceeds max_ttl",
            file=sys.stderr,
        )
        return 2

    claims = _Claims(
        iss=iss,
        aud=aud,
        exp=exp,
        nbf=nbf,
        iat=iat,
        jti=jti,
        org_id=args.org_id,
        user_id=args.user_id,
    )

    private_key = _load_private_key(args.private_key_pem)
    token = _sign_jwt(private_key, claims, args.kid)

    if args.token_only:
        print(token)
    else:
        payload = {
            "mode": args.mode,
            "issuer": iss,
            "audience": aud,
            "iat": iat,
            "exp": exp,
            "exp_minus_iat": exp_minus_iat,
            "max_ttl": args.max_ttl,
            "ttl_violation": ttl_violation,
            "jti": jti,
            "token": token,
        }
        print(json.dumps(payload, indent=2))

    return 0


if __name__ == "__main__":
    raise SystemExit(_main())
