import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

interface StatusBadgeProps {
    status: string;
    className?: string;
}

export function StatusBadge({ status, className }: StatusBadgeProps) {
    let variant: "default" | "secondary" | "destructive" | "outline" = "default";

    switch (status) {
        case "COMPLETED":
            variant = "default"; // Green-ish usually, or customize color
            break;
        case "FAILED":
            variant = "destructive";
            break;
        case "RUNNING":
        case "IN_PROGRESS":
            variant = "secondary";
            break;
        case "PENDING":
            variant = "outline";
            break;
        default:
            variant = "outline";
    }

    // Custom colors for more clarity if needed, using Tailwind classes
    const colorClass =
        status === "COMPLETED" ? "bg-green-500 hover:bg-green-600" :
            status === "FAILED" ? "bg-red-500 hover:bg-red-600" :
                status === "RUNNING" ? "bg-blue-500 hover:bg-blue-600 animate-pulse" :
                    "";

    return (
        <Badge variant={variant} className={cn(colorClass, className)}>
            {status}
        </Badge>
    );
}
