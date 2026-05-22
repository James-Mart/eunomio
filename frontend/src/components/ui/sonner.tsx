import { Toaster as Sonner } from "sonner";

type ToasterProps = React.ComponentProps<typeof Sonner>;

const Toaster = ({ ...props }: ToasterProps) => (
  <Sonner
    className="toaster group"
    toastOptions={{
      classNames: {
        toast:
          "group toast group-[.toaster]:bg-background group-[.toaster]:text-foreground group-[.toaster]:border-border group-[.toaster]:shadow-lg",
        description: "group-[.toast]:text-muted-foreground",
        actionButton: "group-[.toast]:bg-primary group-[.toast]:text-primary-foreground",
        cancelButton: "group-[.toast]:bg-muted group-[.toast]:text-muted-foreground",
        success: "group-[.toast]:text-success group-[.toast]:border-success/30",
        error: "group-[.toast]:text-danger group-[.toast]:border-danger/30",
        warning: "group-[.toast]:text-attention group-[.toast]:border-attention/30",
      },
    }}
    {...props}
  />
);

export { Toaster };
