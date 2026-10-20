const COPY_FEEDBACK_MILLISECONDS = 1600;
const labels = document.currentScript.dataset;

for (const button of document.querySelectorAll("[data-copy]")) {
  const idleLabel = button.textContent;
  button.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(button.dataset.copy);
      button.textContent = labels.copied;
    } catch {
      button.textContent = labels.failed;
    }
    setTimeout(() => { button.textContent = idleLabel; }, COPY_FEEDBACK_MILLISECONDS);
  });
}
