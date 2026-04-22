#!/usr/bin/env python3
"""Mint Ed25519-signed MCP capability envelopes for Tanren phase orchestration."""

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
    phase: str
    spec_id: str
    agent_session_id: str
    capabilities: list[str]


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
        "phase": claims.phase,
        "spec_id": claims.spec_id,
        "agent_session_id": claims.agent_session_id,
        "capabilities": claims.capabilities,
    }

    header_segment = _b64url(json.dumps(header, separators=(",", ":")).encode("utf-8"))
    payload_segment = _b64url(json.dumps(payload, separators=(",", ":")).encode("utf-8"))
    signing_input = f"{header_segment}.{payload_segment}".encode()
    signature = private_key.sign(signing_input)
    signature_segment = _b64url(signature)
    return f"{header_segment}.{payload_segment}.{signature_segment}"


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Mint MCP capability envelope")
    parser.add_argument("--private-key-pem", type=Path, required=True)
    parser.add_argument("--issuer", required=True)
    parser.add_argument("--audience", required=True)
    parser.add_argument("--phase", required=True)
    parser.add_argument("--spec-id", required=True)
    parser.add_argument("--agent-session-id", required=True)
    parser.add_argument("--capabilities", required=True)
    parser.add_argument("--requested-ttl", type=int, default=600)
    parser.add_argument("--max-ttl", type=int, default=900)
    parser.add_argument("--iat", type=int, default=None)
    parser.add_argument("--kid", default="tanren-mcp-capability")
    parser.add_argument("--jti", default=None)
    parser.add_argument("--token-only", action="store_true")
    return parser.parse_args()


def _main() -> int:
    args = _parse_args()

    now = int(time.time())
    iat = args.iat if args.iat is not None else now
    exp = iat + args.requested_ttl
    nbf = iat - 30
    jti = args.jti or str(uuid.uuid4())
    capabilities = [cap.strip() for cap in args.capabilities.split(",") if cap.strip()]

    if not capabilities:
        print("error: --capabilities must include at least one capability tag", file=sys.stderr)
        return 2

    ttl = exp - iat
    if ttl <= 0 or ttl > args.max_ttl:
        print(
            f"error: requested ttl out of bounds (exp-iat={ttl}, max_ttl={args.max_ttl})",
            file=sys.stderr,
        )
        return 2

    print(
        f"iat={iat} exp={exp} exp_minus_iat={ttl} max_ttl={args.max_ttl} phase={args.phase}",
        file=sys.stderr,
    )

    claims = _Claims(
        iss=args.issuer,
        aud=args.audience,
        exp=exp,
        nbf=nbf,
        iat=iat,
        jti=jti,
        phase=args.phase,
        spec_id=args.spec_id,
        agent_session_id=args.agent_session_id,
        capabilities=capabilities,
    )

    private_key = _load_private_key(args.private_key_pem)
    token = _sign_jwt(private_key, claims, args.kid)

    if args.token_only:
        print(token)
    else:
        payload = {
            "issuer": args.issuer,
            "audience": args.audience,
            "phase": args.phase,
            "spec_id": args.spec_id,
            "agent_session_id": args.agent_session_id,
            "capabilities": capabilities,
            "iat": iat,
            "exp": exp,
            "exp_minus_iat": ttl,
            "max_ttl": args.max_ttl,
            "jti": jti,
            "token": token,
        }
        print(json.dumps(payload, indent=2))

    return 0


if __name__ == "__main__":
    raise SystemExit(_main())
