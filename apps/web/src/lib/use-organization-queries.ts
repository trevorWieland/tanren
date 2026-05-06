import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import type {
  CreateOrganizationResponse,
  ListOrganizationsResponse,
} from "@/lib/contract-types";
import {
  createOrganization,
  listAccountOrganizations,
} from "@/app/lib/account-client";

const orgListKey = ["organizations"] as const;

export function useOrganizationList() {
  return useQuery<ListOrganizationsResponse>({
    queryKey: orgListKey,
    queryFn: () => listAccountOrganizations(),
  });
}

export function useCreateOrganization() {
  const queryClient = useQueryClient();
  return useMutation<CreateOrganizationResponse, Error, string>({
    mutationFn: (name: string) => createOrganization(name),
    onSuccess() {
      void queryClient.invalidateQueries({ queryKey: orgListKey });
    },
  });
}
