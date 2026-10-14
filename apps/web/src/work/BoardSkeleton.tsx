import { t } from "../i18n";
import { Skeleton } from "../ui/Skeleton";
import styles from "./Board.module.css";
import { BOARD_COLUMNS } from "./recordStatus";

const CARDS = [0, 1, 2];

/** Gray columns with gray cards while the work loads; the board takes their place. */
export function BoardSkeleton({ narrow }: { narrow: boolean }) {
  const columns = narrow ? BOARD_COLUMNS.slice(0, 1) : BOARD_COLUMNS.slice(0, 4);
  return (
    <div
      className={narrow ? styles.narrow : styles.board}
      role="status"
      aria-label={t("work-loading-board")}
    >
      {columns.map((column) => (
        <div key={column} className={narrow ? styles.cards : styles.column}>
          {CARDS.map((card) => (
            <div key={card} className={styles.skeletonCard}>
              <Skeleton />
              <Skeleton />
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}
