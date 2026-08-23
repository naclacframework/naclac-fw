// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// https://astro.build/config
export default defineConfig({
	site: 'https://docs.naclac.com',
	integrations: [
		starlight({
			title: 'naclac',
			logo: {
				src: './src/assets/naclac-logo.png',
				replacesTitle: false,
			},
			social: [
				{
					icon: 'github',
					label: 'GitHub',
					href: 'https://github.com/naclacframework/naclac-fw',
				},
			],
			customCss: ['./src/styles/naclac-theme.css'],
			// No `sidebar` config: Starlight auto-generates the full nav from
			// src/content/docs/**, ordered by each page's `sidebar.order`
			// frontmatter — new pages need no config changes to appear.
		}),
	],
});
