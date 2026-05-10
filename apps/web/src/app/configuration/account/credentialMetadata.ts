import type { ListUserCredentialsResult } from "@/app/lib/api-contracts";

export type CredentialMetadataKey =
  | "rows"
  | "freshness"
  | "as_of"
  | "generated_at";

export interface CredentialMetadataDescriptor {
  key: CredentialMetadataKey;
  label: string;
  value: string;
}

interface CredentialMetadataCandidate {
  key: CredentialMetadataKey;
  label: string;
  value: string | null | undefined;
}

export function credentialMetadataCandidates(
  readModel: ListUserCredentialsResult | null,
  rowCount: number,
): readonly CredentialMetadataCandidate[] {
  return [
    { key: "rows", label: "rows", value: String(rowCount) },
    { key: "freshness", label: "freshness", value: readModel?.freshness },
    { key: "as_of", label: "as_of", value: readModel?.as_of },
    {
      key: "generated_at",
      label: "generated_at",
      value: readModel?.generated_at,
    },
  ];
}
