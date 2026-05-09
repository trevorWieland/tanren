import * as v from "valibot";

import {
  designatedHostSchema,
  repositoryRefSchema,
  selectAsActiveSchema,
} from "@/app/lib/contracts";

export const connectProjectFormSchema = v.strictObject({
  repository: repositoryRefSchema,
  select_as_active: selectAsActiveSchema,
});

export const createProjectFormSchema = v.strictObject({
  repository: repositoryRefSchema,
  designated_host: designatedHostSchema,
  select_as_active: selectAsActiveSchema,
});
