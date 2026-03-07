import type { Config } from 'tailwindcss';

export default {
	content: ['./src/**/*.{html,js,svelte,ts}'],

	theme: {
		extend: {
			colors: {
				// Opera GX Inspired Neon Green Theme
				'gx': {
					// Primary neon green
					'neon': '#00FF66',
					'neon-dim': '#00CC52',
					'neon-bright': '#33FF85',

					// Background shades (dark)
					'bg': {
						'primary': '#0D0D0D',
						'secondary': '#141414',
						'tertiary': '#1A1A1A',
						'elevated': '#1F1F1F',
						'hover': '#252525',
					},

					// Accent colors
					'accent': {
						'cyan': '#00FFFF',
						'magenta': '#FF00FF',
						'red': '#FF3366',
						'orange': '#FF6600',
						'purple': '#9933FF',
					},

					// Text colors
					'text': {
						'primary': '#FFFFFF',
						'secondary': '#B3B3B3',
						'muted': '#666666',
						'disabled': '#404040',
					},

					// Border colors
					'border': {
						'default': '#2A2A2A',
						'focus': '#00FF66',
						'subtle': '#1F1F1F',
					},

					// Status colors
					'status': {
						'success': '#00FF66',
						'warning': '#FFCC00',
						'error': '#FF3366',
						'info': '#00CCFF',
					}
				}
			},

			fontFamily: {
				'sans': ['Inter', 'system-ui', 'sans-serif'],
				'mono': ['JetBrains Mono', 'Fira Code', 'monospace'],
			},

			boxShadow: {
				'gx-glow': '0 0 20px rgba(0, 255, 102, 0.3)',
				'gx-glow-lg': '0 0 40px rgba(0, 255, 102, 0.4)',
				'gx-glow-sm': '0 0 10px rgba(0, 255, 102, 0.2)',
			},

			animation: {
				'gx-pulse': 'gx-pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite',
				'gx-glow': 'gx-glow 1.5s ease-in-out infinite alternate',
			},

			keyframes: {
				'gx-pulse': {
					'0%, 100%': { opacity: '1' },
					'50%': { opacity: '0.7' },
				},
				'gx-glow': {
					'from': { boxShadow: '0 0 10px rgba(0, 255, 102, 0.2)' },
					'to': { boxShadow: '0 0 25px rgba(0, 255, 102, 0.5)' },
				},
			},

			borderRadius: {
				'gx': '8px',
				'gx-lg': '12px',
			},
		},
	},

	plugins: [],
} satisfies Config;
