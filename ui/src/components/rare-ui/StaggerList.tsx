import { Children, isValidElement, type ReactNode } from "react";
import { motion, useReducedMotion, type HTMLMotionProps } from "motion/react";
import { cn } from "../ui/cn";

const itemVariants = {
  hidden: { opacity: 0, y: 16 },
  visible: {
    opacity: 1,
    y: 0,
    transition: {
      type: "spring" as const,
      stiffness: 320,
      damping: 24,
    },
  },
} as const;

export type StaggerListProps = HTMLMotionProps<"div"> & {
  staggerDelay?: number;
};

export function StaggerList({
  className,
  children,
  staggerDelay,
  ...props
}: StaggerListProps) {
  const reduced = useReducedMotion();

  return (
    <motion.div
      className={cn(className)}
      initial={reduced ? false : "hidden"}
      animate="visible"
      variants={{
        hidden: { opacity: 1 },
        visible: {
          opacity: 1,
          transition: {
            staggerChildren: staggerDelay ?? 0.05,
            delayChildren: 0.02,
          },
        },
      }}
      {...props}
    >
      {Children.toArray(children as ReactNode).map((child, index) => {
        if (!isValidElement(child)) return child;
        return (
          <StaggerItem key={String(child.key ?? index)}>
            {child as ReactNode}
          </StaggerItem>
        );
      })}
    </motion.div>
  );
}

export type StaggerItemProps = HTMLMotionProps<"div">;

export function StaggerItem({ className, children, ...props }: StaggerItemProps) {
  return (
    <motion.div
      className={cn(className)}
      variants={itemVariants}
      {...props}
    >
      {children}
    </motion.div>
  );
}
