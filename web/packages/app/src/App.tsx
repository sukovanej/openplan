import { Link, Outlet } from "react-router-dom"

import { ThemeToggle } from "@/components/theme-toggle"

export function App() {
  return (
    <div className="bg-background text-foreground min-h-screen">
      <header className="border-b">
        <div className="flex items-center gap-3 px-6 py-4">
          <Link to="/" className="text-lg font-semibold tracking-tight">
            open-planner
          </Link>
          <span className="text-muted-foreground text-xs">realtime</span>
          <ThemeToggle className="ml-auto" />
        </div>
      </header>
      <main className="px-6 py-8">
        <Outlet />
      </main>
    </div>
  )
}
