import type { BaseLayoutProps } from 'fumadocs-ui/layouts/shared';
import { siteConfig } from '@/site.config';

export function baseOptions(): BaseLayoutProps {
  return {
    nav: {
      title: siteConfig.title,
    },
    links: [
      {
        text: 'GitHub',
        url: siteConfig.repoUrl,
        external: true,
      },
    ],
  };
}
