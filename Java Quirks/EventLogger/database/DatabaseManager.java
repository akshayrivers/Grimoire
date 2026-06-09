package EventLogger.database;

import EventLogger.model.Event;

public class DatabaseManager {

    public void save(Event e) {

        System.out.println(
                "DB INSERT -> " + e);
    }
}
