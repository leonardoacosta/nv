import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "../lib/utils";

const badgeVariants = cva(
  "inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-semibold transition-colors focus:outline-none focus:ring-2 focus:ring-[hsl(var(--ring))] focus:ring-offset-2",
  {
    variants: {
      variant: {
        default:
          "bg-[rgba(255,255,255,0.06)] text-[hsl(var(--foreground))]",
        destructive:
          "bg-red-700/20 text-red-700",
        success:
          "bg-green-700/20 text-green-700",
        warning:
          "bg-amber-700/20 text-amber-700",
        outline:
          "border border-[hsl(var(--border))] text-[hsl(var(--muted-foreground))]",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  },
);

export interface BadgeProps
  extends React.HTMLAttributes<HTMLDivElement>,
    VariantProps<typeof badgeVariants> {}

function Badge({ className, variant, ...props }: BadgeProps) {
  return (
    <div className={cn(badgeVariants({ variant }), className)} {...props} />
  );
}

export { Badge, badgeVariants };
