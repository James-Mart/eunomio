import Link from 'next/link';
import { siteConfig } from '@/site.config';

export default function HomePage() {
  return (
    <main className="landing">
      <h1 className="landing-title">{siteConfig.title}</h1>
      <p className="landing-desc">{siteConfig.description}</p>
      <div className="landing-actions">
        <Link href="/docs" className="landing-btn landing-btn-primary">
          Read the docs
        </Link>
        <a href={siteConfig.repoUrl} className="landing-btn landing-btn-secondary">
          GitHub
        </a>
      </div>
    </main>
  );
}
