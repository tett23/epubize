import { Link, Outlet } from "react-router";
import { InternalLink } from "./ui";

export function Layout() {
  return (
    <div className="min-h-screen bg-white px-6 py-5 text-neutral-900 dark:bg-neutral-900 dark:text-neutral-100">
      <header className="mb-6 flex items-baseline gap-6 border-b border-neutral-200 pb-3 dark:border-neutral-700">
        <h1 className="text-2xl font-bold">
          <Link to="/">
            epubize <small className="text-sm font-normal text-neutral-500">{import.meta.env.MODE}</small>
          </Link>
        </h1>
        <nav>
          <InternalLink to="/episodes/latest">latest</InternalLink>
        </nav>
      </header>
      <main>
        <Outlet />
      </main>
    </div>
  );
}
