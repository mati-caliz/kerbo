package fixture;

class Switches {
    int withDefault(int k) {
        int x;
        switch (k) {
            case 1:
                x = 10;
                break;
            case 2:
                x = 20;
                break;
            default:
                x = 0;
        }
        return x;
    }

    int rightAfterLiteral(int k) {
        int x = 5;
        switch (k) {
            case 1:
                x = 10;
                break;
            case 2:
                x = 20;
                break;
        }
        return x;
    }

    int notAssignedBefore(int k) {
        int x;
        switch (k) {
            case 1:
                x = 10;
                break;
            case 2:
                x = 20;
                break;
        }
        x = 3;
        return x;
    }

    int assignedEarlier(int k) {
        int x = 1;
        System.out.println();
        switch (k) {
            case 1:
                x = 10;
                break;
        }
        return x;
    }

    int parameter(int k, int y) {
        switch (k) {
            case 1:
                y = 10;
                break;
            default:
                y = 2;
        }
        return y;
    }

    int tooLong(int k) {
        int x;
        switch (k) {
            case 1:
                System.out.println();
                System.out.println();
                x = 10;
                break;
            default:
                x = 0;
        }
        return x;
    }

    int throwing(int k) {
        int x;
        switch (k) {
            case 1:
                x = 10;
                break;
            default:
                throw new IllegalStateException();
        }
        return x;
    }

    int differentVariables(int k) {
        int x;
        int z;
        switch (k) {
            case 1:
                x = 10;
                break;
            default:
                z = 0;
        }
        return k;
    }

    void fallsThrough(int k) {
        int x;
        switch (k) {
            case 1:
                x = 10;
            case 2:
                x = 20;
                break;
            default:
                x = 0;
        }
    }

    int initializedNotLiteral(int k) {
        int x = k * 2;
        switch (k) {
            case 1:
                x = 10;
                break;
        }
        return x;
    }
}
