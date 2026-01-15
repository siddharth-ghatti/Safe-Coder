import { ReactNode } from "react";
import { useUIStore } from "../../stores/uiStore";
import { cn } from "../../lib/utils";

interface AppLayoutProps {
  sidebar: ReactNode;
  main: ReactNode;
  changes: ReactNode;
}

export function AppLayout({ sidebar, main, changes }: AppLayoutProps) {
  const sidebarCollapsed = useUIStore((s) => s.sidebarCollapsed);
  const changesPanelCollapsed = useUIStore((s) => s.changesPanelCollapsed);

  return (
    <div className="h-screen w-screen flex bg-background overflow-hidden">
      {/* Sidebar */}
      <div
        className={cn(
          "h-full border-r border-border bg-card transition-sidebar flex-shrink-0",
          sidebarCollapsed ? "w-0" : "w-64"
        )}
      >
        {!sidebarCollapsed && sidebar}
      </div>

      {/* Main content area */}
      <div className="flex-1 flex flex-col min-w-0">
        {main}
      </div>

      {/* Changes panel */}
      <div
        className={cn(
          "h-full border-l border-border bg-card transition-sidebar flex-shrink-0",
          changesPanelCollapsed ? "w-0" : "w-96"
        )}
      >
        {!changesPanelCollapsed && changes}
      </div>
    </div>
  );
}
