import {
  connectProjectRepositoryInputSchema,
  createProjectInputSchema,
} from "@/app/lib/contracts";

export const connectProjectFormSchema: typeof connectProjectRepositoryInputSchema =
  connectProjectRepositoryInputSchema;

export const createProjectFormSchema: typeof createProjectInputSchema =
  createProjectInputSchema;
