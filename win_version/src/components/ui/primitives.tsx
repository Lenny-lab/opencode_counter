import { cva } from "class-variance-authority";
import type { ButtonHTMLAttributes, HTMLAttributes, ReactNode } from "react";

import { cn } from "@/lib/utils";

const cardVariants = cva("rounded-lg border border-white/10 bg-white/[0.035] text-foreground", {
  variants: {
    padded: { true: "p-3", false: "" },
  },
  defaultVariants: { padded: true },
});

type CardProps = HTMLAttributes<HTMLDivElement> & {
  title?: ReactNode;
  action?: ReactNode;
  /** Panels that manage their own inner spacing opt out. */
  padded?: boolean;
};

/** The bordered panel every figure sits in. */
export function Card({ title, action, className, padded = true, children, ...rest }: CardProps) {
  return (
    <section className={cn(cardVariants({ padded }), className)} {...rest}>
      {title !== undefined && (
        <header className="mb-2 flex items-baseline justify-between gap-2">
          <h2 className="text-[11px] font-medium tracking-wide text-muted uppercase">{title}</h2>
          {action}
        </header>
      )}
      {children}
    </section>
  );
}

type ButtonVariant = "ghost" | "solid" | "outline";
type ButtonSize = "sm" | "md" | "icon";

const buttonVariants = cva(
  "inline-flex items-center justify-center gap-1.5 rounded-md text-xs transition-colors select-none disabled:pointer-events-none disabled:opacity-40 outline-none focus-visible:ring-2 focus-visible:ring-primary/60",
  {
    variants: {
      variant: {
        ghost: "text-muted hover:bg-white/8 hover:text-foreground",
        solid: "bg-primary/15 text-primary hover:bg-primary/25",
        outline: "border border-white/12 text-foreground/80 hover:bg-white/8",
      },
      size: {
        sm: "h-7 px-2.5",
        md: "h-8 px-3",
        icon: "h-7 w-7",
      },
    },
    defaultVariants: { variant: "ghost", size: "sm" },
  },
);

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: ButtonVariant;
  size?: ButtonSize;
};

export function Button({ className, variant, size, ...rest }: ButtonProps) {
  return <button type="button" className={cn(buttonVariants({ variant, size }), className)} {...rest} />;
}

/** A label plus a figure, the unit the overview and popover are built from. */
export function Stat({
  label,
  value,
  tone,
  hint,
}: {
  label: string;
  value: ReactNode;
  tone?: string;
  hint?: string;
}) {
  return (
    <div className="flex flex-col gap-0.5">
      <span className="text-[10px] tracking-wide text-muted uppercase">{label}</span>
      <span className="tnum text-[15px] font-semibold" style={tone ? { color: tone } : undefined}>
        {value}
      </span>
      {hint && <span className="text-[10px] text-muted">{hint}</span>}
    </div>
  );
}

/** Horizontal magnitude bar used in the model, provider and tool lists. */
export function Bar({ ratio, color }: { ratio: number; color: string }) {
  const width = `${Math.max(0, Math.min(1, ratio)) * 100}%`;
  return (
    <div className="h-1 w-full overflow-hidden rounded-full bg-white/8">
      <div className="h-full rounded-full" style={{ width, backgroundColor: color }} />
    </div>
  );
}

export function Empty({ children }: { children: ReactNode }) {
  return (
    <p className="py-6 text-center text-[11px] text-muted">
      {children}
    </p>
  );
}

export function Spinner({ className }: { className?: string }) {
  return (
    <svg className={cn("size-4 animate-spin", className)} viewBox="0 0 24 24" fill="none" aria-hidden>
      <circle cx="12" cy="12" r="9" stroke="currentColor" strokeOpacity="0.25" strokeWidth="3" />
      <path d="M21 12a9 9 0 0 0-9-9" stroke="currentColor" strokeWidth="3" strokeLinecap="round" />
    </svg>
  );
}
