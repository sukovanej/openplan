import { Link, Outlet } from "react-router-dom"

import { ConnectionStatus } from "@/components/connection-status"
import { ThemeToggle } from "@/components/theme-toggle"

export function App() {
  return (
    <div className="bg-background text-foreground min-h-screen">
      <header className="border-b">
        <div className="flex items-center gap-3 px-6 py-4">
          <Link to="/" className="text-2xl font-semibold tracking-tight">
            Open Plan
          </Link>
          <ConnectionStatus />
          <ThemeToggle className="ml-auto" />
        </div>
      </header>
      <main className="px-6 py-8">
        <Outlet />
      </main>
    </div>
  )
}
