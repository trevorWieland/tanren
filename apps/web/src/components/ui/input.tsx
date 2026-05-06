"use client";

import { forwardRef } from "react";
import type { InputHTMLAttributes, ReactNode } from "react";

export interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  children?: ReactNode;
}

const baseClass =
  "rounded-md border border-[--color-border] bg-[--color-bg-surface] px-3 py-2 text-base text-[--color-fg-default] focus:outline-none focus:ring-2 focus:ring-[--color-accent]";

export const Input = forwardRef<HTMLInputElement, InputProps>(function Input(
  { className, ...rest },
  ref,
) {
  return <input ref={ref} className={className ?? baseClass} {...rest} />;
});
