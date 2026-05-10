import type { FormEvent, ReactNode } from "react";

export function RoleCard(props: {
  title: string;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  children: ReactNode;
}): ReactNode {
  return (
    <form
      className="rounded-md border border-[--color-border] bg-[--color-bg-surface] p-4"
      onSubmit={props.onSubmit}
    >
      <h2 className="mb-2 text-lg font-medium">{props.title}</h2>
      <div className="space-y-2">{props.children}</div>
      <button
        className="mt-3 rounded border border-[--color-border] px-3 py-1 text-sm"
        type="submit"
      >
        Run
      </button>
    </form>
  );
}

export function LabeledInput(props: {
  name: string;
  label: string;
  placeholder: string;
}): ReactNode {
  return (
    <label className="grid gap-1 text-sm">
      <span>{props.label}</span>
      <input
        className="rounded border border-[--color-border] bg-transparent px-2 py-1"
        name={props.name}
        placeholder={props.placeholder}
      />
    </label>
  );
}

export function ScopeFields(props: {
  prefix: string;
  legend?: string;
}): ReactNode {
  return (
    <fieldset className="grid gap-2">
      <legend className="text-xs uppercase text-[--color-fg-muted]">
        {props.legend ?? "Scope"}
      </legend>
      <label className="grid gap-1 text-sm">
        <span>Scope kind</span>
        <select
          className="rounded border border-[--color-border] bg-transparent px-2 py-1"
          defaultValue="account"
          name={`${props.prefix}kind`}
        >
          <option value="account">account</option>
          <option value="organization">organization</option>
          <option value="project">project</option>
        </select>
      </label>
      <LabeledInput
        name={`${props.prefix}id`}
        label="Scope id"
        placeholder="uuid"
      />
    </fieldset>
  );
}

export function PrincipalFields(props: {
  prefix: string;
  allowRolePrincipal?: boolean;
}): ReactNode {
  return (
    <fieldset className="grid gap-2">
      <legend className="text-xs uppercase text-[--color-fg-muted]">
        Principal
      </legend>
      <label className="grid gap-1 text-sm">
        <span>Principal kind</span>
        <select
          className="rounded border border-[--color-border] bg-transparent px-2 py-1"
          defaultValue="account"
          name={`${props.prefix}kind`}
        >
          <option value="account">account</option>
          {props.allowRolePrincipal ? <option value="role">role</option> : null}
        </select>
      </label>
      <LabeledInput
        name={`${props.prefix}id`}
        label="Principal id"
        placeholder="uuid"
      />
    </fieldset>
  );
}
