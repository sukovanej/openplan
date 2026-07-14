import { Link, Outlet } from "react-router-dom"

import { ThemeToggle } from "@/components/theme-toggle"

export function App() {
  return (
    <div className="bg-background text-foreground min-h-screen">
      <header className="border-b">
        <div className="mx-auto flex max-w-3xl items-center gap-3 px-4 py-4">
          <Link to="/" className="text-lg font-semibold tracking-tight">
            open-planner
          </Link>
          <span className="text-muted-foreground text-xs">realtime</span>
          <ThemeToggle className="ml-auto" />
        </div>
      </header>
      <main className="mx-auto max-w-3xl px-4 py-8">
        <Outlet />
      </main>
    </div>
  )
}
