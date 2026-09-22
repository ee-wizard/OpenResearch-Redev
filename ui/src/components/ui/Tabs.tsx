import {
  createContext,
  useContext,
  useState,
  forwardRef,
  useId,
  type ReactNode,
} from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { motion, useReducedMotion } from "motion/react";
import { cn } from "./cn";

const TabsContext = createContext<{
  value: string;
  onValueChange: (value: string) => void;
  baseId: string;
} | null>(null);

function useTabs() {
  const ctx = useContext(TabsContext);
  if (!ctx) throw new Error("Tabs parts must be rendered inside <Tabs>");
  return ctx;
}

export type TabsProps = {
  value?: string;
  defaultValue?: string;
  onValueChange?: (value: string) => void;
  children: ReactNode;
  className?: string;
};

export function Tabs({
  value,
  defaultValue,
  onValueChange,
  children,
  className,
}: TabsProps) {
  const baseId = useId();
  const [internalValue, setInternalValue] = useState(defaultValue ?? "");
  const activeValue = value ?? internalValue;

  return (
    <TabsContext.Provider
      value={{
        value: activeValue,
        onValueChange: (next) => {
          setInternalValue(next);
          onValueChange?.(next);
        },
        baseId,
      }}
    >
      <div className={cn("flex flex-col gap-2", className)}>{children}</div>
    </TabsContext.Provider>
  );
}

const listVariants = cva(
  "inline-flex items-center gap-1 p-1 bg-surface rounded-xl border border-border",
  {
    variants: {
      size: {
        default: "h-9",
        sm: "h-8",
      },
    },
    defaultVariants: { size: "default" },
  },
);

export type TabsListProps = React.ComponentProps<"div"> &
  VariantProps<typeof listVariants>;

export const TabsList = forwardRef<HTMLDivElement, TabsListProps>(function TabsList(
  { className, size = "default", ...props },
  ref,
) {
  return (
    <div
      ref={ref}
      role="tablist"
      className={cn(listVariants({ size }), className)}
      {...props}
    />
  );
});

const triggerVariants = cva(
  "relative z-1 inline-flex items-center justify-center gap-1.5 whitespace-nowrap rounded-lg px-3 py-1 text-sm font-medium transition-colors focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-text focus-visible:outline-offset-2 disabled:cursor-default disabled:opacity-45",
  {
    variants: {
      active: {
        true: "text-background",
        false: "text-subtext hover:text-text",
      },
    },
    defaultVariants: { active: false },
  },
);

export type TabsTriggerProps = React.ComponentProps<"button"> & {
  value: string;
};

export const TabsTrigger = forwardRef<HTMLButtonElement, TabsTriggerProps>(function TabsTrigger(
  { value, className, children, ...props },
  ref,
) {
  const tabs = useTabs();
  const reduced = useReducedMotion();
  const active = tabs.value === value;

  return (
    <button
      ref={ref}
      type="button"
      role="tab"
      aria-selected={active}
      aria-controls={`${tabs.baseId}-${value}-panel`}
      id={`${tabs.baseId}-${value}-trigger`}
      className={cn(triggerVariants({ active }), className)}
      onClick={() => tabs.onValueChange(value)}
      {...props}
    >
      {active && (
        <motion.span
          layoutId={`${tabs.baseId}-indicator`}
          className="absolute inset-0 z-[-1] rounded-lg bg-primary"
          transition={
            reduced
              ? { duration: 0 }
              : { type: "spring", stiffness: 380, damping: 30 }
          }
        />
      )}
      {children}
    </button>
  );
});

export type TabsContentProps = React.ComponentProps<"div"> & {
  value: string;
};

export const TabsContent = forwardRef<HTMLDivElement, TabsContentProps>(function TabsContent(
  { value, className, children, ...props },
  ref,
) {
  const tabs = useTabs();
  const active = tabs.value === value;
  if (!active) return null;

  return (
    <div
      ref={ref}
      role="tabpanel"
      id={`${tabs.baseId}-${value}-panel`}
      aria-labelledby={`${tabs.baseId}-${value}-trigger`}
      className={cn("outline-none focus-visible:ring-2 focus-visible:ring-primary/30 rounded-xl", className)}
      tabIndex={0}
      {...props}
    >
      {children}
    </div>
  );
});
