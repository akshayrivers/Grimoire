package EventLogger.consumer;

import EventLogger.database.DatabaseManager;
import EventLogger.model.Event;

public class DatabaseConsumer implements LogListener {

    private final DatabaseManager db;

    public DatabaseConsumer(DatabaseManager db) {
        this.db = db;
    }

    @Override
    public void onEvent(Event e) {
        db.save(e);
    }
}
