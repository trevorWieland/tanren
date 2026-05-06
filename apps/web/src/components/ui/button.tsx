"use client";

import { forwardRef } from "react";
import type { ButtonHTMLAttributes, ReactNode } from "react";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  children?: ReactNode;
}

const baseClass =
  "rounded-md border border-[--color-border] bg-[--color-accent] px-4 py-2 text-base font-medium text-[--color-accent-fg] transition-colors hover:bg-[--color-accent-hover] disabled:opacity-60";

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  function Button({ className, children, ...rest }, ref) {
    return (
      <button ref={ref} className={className ?? baseClass} {...rest}>
        {children}
      </button>
    );
  },
);
