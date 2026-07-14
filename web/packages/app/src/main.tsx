import { StrictMode } from "react"
import { createRoot } from "react-dom/client"
import { createBrowserRouter, RouterProvider } from "react-router-dom"

import { App } from "./App"
import { startRealtime } from "./lib/realtime"
import { DetailRoute } from "./routes/detail"
import { ListRoute } from "./routes/list"

import "./index.css"

const router = createBrowserRouter([
  {
    element: <App />,
    children: [
      { path: "/", element: <ListRoute /> },
      { path: "/task/:id", element: <DetailRoute /> },
    ],
  },
])

const root = document.getElementById("root")
if (root === null) throw new Error("missing #root element")

startRealtime()

createRoot(root).render(
  <StrictMode>
    <RouterProvider router={router} />
  </StrictMode>,
)
