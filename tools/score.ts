/**
 * Default Scoring Function for Sorex
 *
 * This is the built-in scoring function that mirrors the Lean-proven constants
 * from src/scoring/core.rs. Users can override this by providing their own
 * scoring.ts file via `sorex index --ranking ./scoring.ts`.
 *
 * The function receives a ScoringContext for each (term, doc, match) tuple
 * and returns an integer score (higher = better ranking).
 */

/**
 * Context passed to the scoring function for each term occurrence.
 */
export interface ScoringContext {
	/** The vocabulary term being indexed */
	term: string;
	/** Document metadata */
	doc: {
		id: number;
		title: string;
		excerpt: string;
		href: string;
		type: string; // "post" | "page"
		category: string | null;
		author: string | null;
		tags: string[];
	};
	/** Match location within the document */
	match: {
		fieldType: 'title' | 'heading' | 'content';
		headingLevel: number; // 0=title, 2=h2, 3=h3, etc.
		sectionId: string | null;
		offset: number;
		textLength: number;
	};
}

/**
 * Field type base scores (from Lean-proven constants in Scoring.lean).
 *
 * These values ensure title matches always beat heading matches,
 * and heading matches always beat content matches, even with
 * maximum position bonus applied.
 *
 * Invariant (proven in Lean):
 *   TITLE - MAX_POSITION_BONUS > HEADING + MAX_POSITION_BONUS
 *   HEADING - MAX_POSITION_BONUS > CONTENT + MAX_POSITION_BONUS
 */
const TITLE = 1000;
const HEADING = 100;
const CONTENT = 10;
const MAX_POSITION_BONUS = 5;

/**
 * Default scoring function - mirrors src/scoring/core.rs logic.
 *
 * @param ctx - The scoring context for this term occurrence
 * @returns Integer score (higher = better)
 */
export default function score(ctx: ScoringContext): number {
	// Base score by field type
	let score: number;
	switch (ctx.match.fieldType) {
		case 'title':
			score = TITLE;
			break;
		case 'heading':
			score = HEADING;
			break;
		default:
			score = CONTENT;
	}

	// Position bonus: earlier in text = higher score
	// This provides a small tie-breaker within the same field type
	const positionRatio =
		ctx.match.textLength > 0 ? (ctx.match.textLength - ctx.match.offset) / ctx.match.textLength : 1;
	score += Math.floor(MAX_POSITION_BONUS * positionRatio);

	return score;
}

// Re-export for use in custom ranking functions
export { score as defaultScore, TITLE, HEADING, CONTENT, MAX_POSITION_BONUS };
