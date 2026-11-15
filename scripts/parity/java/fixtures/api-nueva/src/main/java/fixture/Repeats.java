package fixture;

class Repeats {
    String closed(int n) {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i <= n; i++) {
            sb.append('x');
        }
        return sb.toString();
    }

    String down(int n) {
        StringBuilder sb = new StringBuilder();
        for (int i = n; i > 0; i--) {
            sb.append('x');
        }
        return sb.toString();
    }

    String downClosed(int n) {
        StringBuilder sb = new StringBuilder();
        for (int i = n; i >= 0; i--) {
            sb.append('x');
        }
        return sb.toString();
    }

    String notEqual(int n) {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i != n; i++) {
            sb.append('x');
        }
        return sb.toString();
    }

    String stepOne(int n) {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < n; i += 1) {
            sb.append("ab");
        }
        return sb.toString();
    }

    String reversed(int n) {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; n > i; i++) {
            sb.append("ab");
        }
        return sb.toString();
    }
}
